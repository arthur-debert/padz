package store

import (
	"fmt"
	"sort"
	"strconv"
	"strings"
	"time"
)

// ScopedScratch represents a scratch with scope information
type ScopedScratch struct {
	*Scratch
	Scope    string // The scope this scratch belongs to (e.g., "global", "myproject")
	ScopedID string // Scoped ID in format "scope:index" (e.g., "global:1", "myproject:2")
	Index    int    // Index within the scope (1-based, corresponds to the number after the colon in ScopedID)
}

// MergedStore combines multiple stores for cross-scope operations
type MergedStore struct {
	stores map[string]*Store
	sm     *StoreManager
}

// NewMergedStore creates a new MergedStore from multiple scopes
func NewMergedStore(sm *StoreManager, scopes []string, workingDirs map[string]string) (*MergedStore, error) {
	stores := make(map[string]*Store)

	for _, scope := range scopes {
		workingDir := workingDirs[scope]
		if workingDir == "" {
			workingDir = "." // Default to current directory
		}

		var store *Store
		var err error
		if scope == "global" {
			store, err = sm.GetGlobalStore()
		} else {
			store, err = sm.GetProjectStore(scope, workingDir)
		}
		if err != nil {
			return nil, fmt.Errorf("failed to get store for scope %s: %w", scope, err)
		}
		stores[scope] = store
	}

	return &MergedStore{
		stores: stores,
		sm:     sm,
	}, nil
}

// GetAllScratches returns all scratches from all scopes with scope information
func (ms *MergedStore) GetAllScratches(includeDeleted bool) []ScopedScratch {
	var result []ScopedScratch

	for scope, store := range ms.stores {
		// Use centralized filtering and sorting logic
		filtered := ms.getSortedFilteredScratches(store, includeDeleted)

		// Convert to ScopedScratch with proper indexing (per-scope)
		for i, scratch := range filtered {
			// Use simplified scope for ScopedID (e.g., "proj1" instead of "project:proj1")
			simplifiedScope := scope
			if strings.HasPrefix(scope, "project:") {
				simplifiedScope = strings.TrimPrefix(scope, "project:")
			}

			scopedScratch := ScopedScratch{
				Scratch:  &scratch,
				Scope:    scope,
				ScopedID: fmt.Sprintf("%s:%d", simplifiedScope, i+1),
				Index:    i + 1,
			}
			result = append(result, scopedScratch)
		}
	}

	// Sort all scratches by most recent activity across scopes
	sort.Slice(result, func(i, j int) bool {
		timeI := getMostRecentTime(result[i].Scratch)
		timeJ := getMostRecentTime(result[j].Scratch)
		return timeI.After(timeJ)
	})

	return result
}

// GetScopedScratch resolves a scoped ID (like "global:1" or "padz:1") to a specific scratch
func (ms *MergedStore) GetScopedScratch(scopedID string) (*ScopedScratch, error) {
	parts := strings.SplitN(scopedID, ":", 2)
	if len(parts) != 2 {
		return nil, fmt.Errorf("invalid scoped ID format: %s (expected format: scope:index)", scopedID)
	}

	scope := parts[0]
	indexStr := parts[1]

	// Normalize scope format for store lookup
	storeKey := scope
	if scope != "global" && !strings.HasPrefix(scope, "project:") {
		storeKey = fmt.Sprintf("project:%s", scope)
	}

	store, exists := ms.stores[storeKey]
	if !exists {
		return nil, fmt.Errorf("scope %s not found in merged store", scope)
	}

	// Convert index to integer
	index, err := strconv.Atoi(indexStr)
	if err != nil {
		return nil, fmt.Errorf("invalid index format %s in scope %s: %w", indexStr, scope, err)
	}

	// Get sorted and filtered scratches using the same logic as GetAllScratches
	filtered := ms.getSortedFilteredScratches(store, false)

	if index < 1 || index > len(filtered) {
		return nil, fmt.Errorf("index %d out of range in scope %s (1-%d)", index, scope, len(filtered))
	}

	scratch := &filtered[index-1]

	return &ScopedScratch{
		Scratch:  scratch,
		Scope:    storeKey, // Use the normalized scope for consistency
		ScopedID: scopedID,
		Index:    index,
	}, nil
}

// GetAvailableScopes returns all scopes available in this merged store
func (ms *MergedStore) GetAvailableScopes() []string {
	scopes := make([]string, 0, len(ms.stores))
	for scope := range ms.stores {
		scopes = append(scopes, scope)
	}
	return sortScopes(scopes)
}

// ValidateNoScopedIDs ensures that all IDs are plain (not scoped) for single-scope operations
func ValidateNoScopedIDs(ids []string) error {
	var scopedIDs []string
	for _, id := range ids {
		if strings.Contains(id, ":") {
			scopedIDs = append(scopedIDs, id)
		}
	}

	if len(scopedIDs) > 0 {
		return fmt.Errorf("scoped IDs not allowed in multi-ID operations within single scope: %s",
			strings.Join(scopedIDs, ", "))
	}

	return nil
}

// ExtractScopesFromIDs extracts unique scopes from a list of potentially scoped IDs
func ExtractScopesFromIDs(ids []string) []string {
	scopeSet := make(map[string]bool)

	for _, id := range ids {
		if strings.Contains(id, ":") {
			parts := strings.SplitN(id, ":", 2)
			if len(parts) == 2 {
				scopeSet[parts[0]] = true
			}
		}
	}

	scopes := make([]string, 0, len(scopeSet))
	for scope := range scopeSet {
		scopes = append(scopes, scope)
	}
	return sortScopes(scopes)
}

// getSortedFilteredScratches centralizes the filtering and sorting logic used by both
// GetAllScratches and GetScopedScratch to ensure consistency
func (ms *MergedStore) getSortedFilteredScratches(store *Store, includeDeleted bool) []Scratch {
	allScratches := store.GetScratches()

	// Filter scratches based on requirements
	var filtered []Scratch
	for _, scratch := range allScratches {
		if includeDeleted || !scratch.IsDeleted {
			filtered = append(filtered, scratch)
		}
	}

	// Sort by most recent time
	sort.Slice(filtered, func(i, j int) bool {
		return getMostRecentTime(&filtered[i]).After(getMostRecentTime(&filtered[j]))
	})

	return filtered
}

// sortScopes provides consistent sorting for scope names
func sortScopes(scopes []string) []string {
	sort.Strings(scopes)
	return scopes
}

// getMostRecentTime returns the most recent time for a scratch (creation, update, or deletion)
func getMostRecentTime(scratch *Scratch) time.Time {
	mostRecent := scratch.CreatedAt

	if scratch.UpdatedAt.After(mostRecent) {
		mostRecent = scratch.UpdatedAt
	}

	if scratch.IsDeleted && scratch.DeletedAt != nil && scratch.DeletedAt.After(mostRecent) {
		mostRecent = *scratch.DeletedAt
	}

	return mostRecent
}
