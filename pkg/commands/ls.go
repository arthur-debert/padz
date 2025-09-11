package commands

import (
	"fmt"
	"sort"
	"strconv"
	"strings"

	"github.com/arthur-debert/padz/pkg/store"
)

// ListMode defines how to filter deleted items
type ListMode string

const (
	ListModeActive  ListMode = "active"  // Only non-deleted items (default)
	ListModeDeleted ListMode = "deleted" // Only deleted items
	ListModeAll     ListMode = "all"     // Both deleted and non-deleted items
)

func Ls(s *store.Store, all, global bool, project string) []store.Scratch {
	// Default behavior - exclude deleted items
	return LsWithMode(s, all, global, project, ListModeActive)
}

// LsWithStoreManager lists scratches using the new StoreManager approach
func LsWithStoreManager(workingDir string, globalFlag, allFlag bool, mode ListMode) (interface{}, error) {
	sm := store.NewStoreManager()

	if allFlag {
		// Cross-scope operation: get all available scopes
		scopes := []string{"global"} // Always include global

		// Find project scopes by examining working directory
		if !globalFlag {
			// Try to get current project scope
			_, currentScope, err := sm.GetCurrentStore(workingDir, false)
			if err == nil && currentScope != "global" {
				// Add current project scope
				scopes = append(scopes, currentScope)
			}

			// TODO: In future, we might want to discover all project scopes
			// For now, we include global + current project when --all is used
		}

		// Create working directories map
		workingDirs := make(map[string]string)
		workingDirs["global"] = ""
		if len(scopes) > 1 {
			workingDirs[scopes[1]] = workingDir // Current project
		}

		// Create merged store for cross-scope operations
		mergedStore, err := store.NewMergedStore(sm, scopes, workingDirs)
		if err != nil {
			return nil, fmt.Errorf("failed to create merged store: %w", err)
		}

		// Get scoped scratches
		includeDeleted := (mode == ListModeDeleted || mode == ListModeAll)
		scopedScratches := mergedStore.GetAllScratches(includeDeleted)

		// Filter based on mode
		var filtered []store.ScopedScratch
		for _, scratch := range scopedScratches {
			switch mode {
			case ListModeActive:
				if !scratch.IsDeleted {
					filtered = append(filtered, scratch)
				}
			case ListModeDeleted:
				if scratch.IsDeleted {
					filtered = append(filtered, scratch)
				}
			case ListModeAll:
				filtered = append(filtered, scratch)
			}
		}

		return filtered, nil
	} else {
		// Single-scope operation
		currentStore, currentScope, err := sm.GetCurrentStore(workingDir, globalFlag)
		if err != nil {
			return nil, fmt.Errorf("failed to get current store: %w", err)
		}

		// Use the existing LsWithMode logic but with the scope-specific store
		// Extract project name from scope (e.g., "project:padz" -> "padz", "global" -> "global")
		projectName := currentScope
		if strings.HasPrefix(currentScope, "project:") {
			projectName = strings.TrimPrefix(currentScope, "project:")
		}
		scratches := LsWithMode(currentStore, false, currentScope == "global", projectName, mode)
		return scratches, nil
	}
}

// LsWithMode lists scratches with delete filtering options
func LsWithMode(s *store.Store, all, global bool, project string, mode ListMode) []store.Scratch {
	scratches := s.GetScratches()

	var filtered []store.Scratch
	for _, scratch := range scratches {
		// Filter by deletion status based on mode
		switch mode {
		case ListModeActive:
			if scratch.IsDeleted {
				continue
			}
		case ListModeDeleted:
			if !scratch.IsDeleted {
				continue
			}
		case ListModeAll:
			// Include everything
		}

		// Filter by project/global
		if all {
			filtered = append(filtered, scratch)
		} else if global && scratch.Project == "global" {
			filtered = append(filtered, scratch)
		} else if !global && scratch.Project == project {
			filtered = append(filtered, scratch)
		}
	}

	// For ListModeAll, sort by most recent activity to intermingle active and deleted items
	if mode == ListModeAll {
		return sortByMostRecentActivity(filtered)
	}

	return sortByCreatedAtDesc(filtered)
}

func sortByCreatedAtDesc(scratches []store.Scratch) []store.Scratch {
	sorted := make([]store.Scratch, len(scratches))
	copy(sorted, scratches)
	sort.Slice(sorted, func(i, j int) bool {
		return sorted[i].CreatedAt.After(sorted[j].CreatedAt)
	})
	return sorted
}

// sortByMostRecentActivity sorts scratches by their most recent activity
// (creation date for active items, deletion date for deleted items)
func sortByMostRecentActivity(scratches []store.Scratch) []store.Scratch {
	sorted := make([]store.Scratch, len(scratches))
	copy(sorted, scratches)
	sort.Slice(sorted, func(i, j int) bool {
		// Get the most recent activity date for each scratch
		iTime := sorted[i].CreatedAt
		if sorted[i].IsDeleted && sorted[i].DeletedAt != nil {
			iTime = *sorted[i].DeletedAt
		}

		jTime := sorted[j].CreatedAt
		if sorted[j].IsDeleted && sorted[j].DeletedAt != nil {
			jTime = *sorted[j].DeletedAt
		}

		return iTime.After(jTime)
	})
	return sorted
}

func GetScratchByIndex(s *store.Store, all, global bool, project string, indexStr string) (*store.Scratch, error) {
	// When looking for deleted items (d1, d2, etc), we need to get all items including deleted
	var scratches []store.Scratch
	if len(indexStr) > 1 && indexStr[0] == 'd' {
		scratches = LsWithMode(s, all, global, project, ListModeAll)
	} else {
		scratches = Ls(s, all, global, project)
	}

	// Check if it's a deleted index (d1, d2, etc)
	if len(indexStr) > 1 && indexStr[0] == 'd' {
		deletedIndexStr := indexStr[1:]
		deletedIndex, err := strconv.Atoi(deletedIndexStr)
		if err != nil {
			return nil, fmt.Errorf("invalid deleted index: %s", indexStr)
		}

		// Get deleted items sorted by deletion time (newest first)
		var deletedScratches []store.Scratch
		for _, scratch := range scratches {
			if scratch.IsDeleted {
				deletedScratches = append(deletedScratches, scratch)
			}
		}

		// Sort by DeletedAt descending (newest first)
		sort.Slice(deletedScratches, func(i, j int) bool {
			if deletedScratches[i].DeletedAt == nil || deletedScratches[j].DeletedAt == nil {
				return false
			}
			return deletedScratches[i].DeletedAt.After(*deletedScratches[j].DeletedAt)
		})

		if deletedIndex < 1 || deletedIndex > len(deletedScratches) {
			return nil, fmt.Errorf("deleted index out of range: %s", indexStr)
		}

		return &deletedScratches[deletedIndex-1], nil
	}

	// Check if it's a pinned index (p1, p2, etc)
	if len(indexStr) > 1 && indexStr[0] == 'p' {
		pinnedIndexStr := indexStr[1:]
		pinnedIndex, err := strconv.Atoi(pinnedIndexStr)
		if err != nil {
			return nil, fmt.Errorf("invalid pinned index: %s", indexStr)
		}

		// Count pinned items
		pinnedCount := 0
		for i, scratch := range scratches {
			if scratch.IsPinned {
				pinnedCount++
				if pinnedCount == pinnedIndex {
					return &scratches[i], nil
				}
			}
		}
		return nil, fmt.Errorf("pinned index out of range: %s", indexStr)
	}

	// Regular index - exclude deleted items
	var activeScratches []store.Scratch
	for _, scratch := range scratches {
		if !scratch.IsDeleted {
			activeScratches = append(activeScratches, scratch)
		}
	}

	index, err := strconv.Atoi(indexStr)
	if err != nil {
		return nil, fmt.Errorf("invalid index: %s", indexStr)
	}

	if index < 1 || index > len(activeScratches) {
		return nil, fmt.Errorf("index out of range: %d", index)
	}

	return &activeScratches[index-1], nil
}

// ResolveScratchID resolves various ID formats (index, pinned index, deleted index, hash) to a scratch
func ResolveScratchID(s *store.Store, all, global bool, project string, id string) (*store.Scratch, error) {
	// Try as index (including pinned index and deleted index)
	scratch, err := GetScratchByIndex(s, all, global, project, id)
	if err == nil {
		return scratch, nil
	}

	// Try as hash ID
	scratches := s.GetScratches()
	for i := range scratches {
		if strings.HasPrefix(scratches[i].ID, id) {
			return &scratches[i], nil
		}
	}

	return nil, fmt.Errorf("scratch not found: %s", id)
}

// ResolveScratchWithStoreManager resolves various ID formats including scoped IDs (scope:index) using StoreManager
func ResolveScratchWithStoreManager(workingDir string, globalFlag bool, id string) (*store.ScopedScratch, error) {
	sm := store.NewStoreManager()

	// Check if this is a scoped ID (contains colon)
	if strings.Contains(id, ":") {
		parts := strings.SplitN(id, ":", 2)
		if len(parts) == 2 {
			scope := parts[0]

			// Normalize scope format for MergedStore
			normalizedScope := scope
			if scope != "global" && !strings.HasPrefix(scope, "project:") {
				normalizedScope = fmt.Sprintf("project:%s", scope)
			}

			// Create working directories map for the specific scope
			workingDirs := make(map[string]string)
			if normalizedScope == "global" {
				workingDirs[normalizedScope] = ""
			} else {
				// For project scopes, we need to find the project directory
				// For now, assume current working directory belongs to the requested scope
				workingDirs[normalizedScope] = workingDir
			}

			// Create merged store with just this scope
			mergedStore, err := store.NewMergedStore(sm, []string{normalizedScope}, workingDirs)
			if err != nil {
				return nil, fmt.Errorf("failed to create merged store for scope %s: %w", scope, err)
			}

			// Use the MergedStore's scoped ID resolution
			return mergedStore.GetScopedScratch(id)
		}
	}

	// Non-scoped ID: resolve in current scope
	currentStore, currentScope, err := sm.GetCurrentStore(workingDir, globalFlag)
	if err != nil {
		return nil, fmt.Errorf("failed to get current store: %w", err)
	}

	// Extract project name from scope
	projectName := currentScope
	if strings.HasPrefix(currentScope, "project:") {
		projectName = strings.TrimPrefix(currentScope, "project:")
	}

	// Use existing resolution logic
	scratch, err := ResolveScratchID(currentStore, false, currentScope == "global", projectName, id)
	if err != nil {
		return nil, err
	}

	// Convert to ScopedScratch format
	// We need to determine the index within the current scope
	scratches := LsWithMode(currentStore, false, currentScope == "global", projectName, ListModeActive)
	for i, s := range scratches {
		if s.ID == scratch.ID {
			return &store.ScopedScratch{
				Scratch:  scratch,
				Scope:    currentScope,
				ScopedID: fmt.Sprintf("%s:%d", currentScope, i+1),
				Index:    i + 1,
			}, nil
		}
	}

	return nil, fmt.Errorf("scratch not found in current scope: %s", id)
}
