package store

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

// ScopedID represents a parsed ID with scope and user ID
type ScopedID struct {
	Scope  string
	UserID int
}

// Dispatcher coordinates operations across multiple single-scope stores
type Dispatcher struct {
	stores map[string]*Store // cached stores by scope name
}

// NewDispatcher creates a new dispatcher
func NewDispatcher() *Dispatcher {
	return &Dispatcher{
		stores: make(map[string]*Store),
	}
}

// GetStore returns a store for the given scope, creating it if necessary
func (d *Dispatcher) GetStore(scope string) (*Store, error) {
	// Check cache first
	if store, exists := d.stores[scope]; exists {
		return store, nil
	}

	// Create new store
	storePath, err := GetStorePath(scope)
	if err != nil {
		return nil, fmt.Errorf("failed to get store path for scope %s: %w", scope, err)
	}

	store, err := NewStore(storePath)
	if err != nil {
		return nil, fmt.Errorf("failed to create store for scope %s: %w", scope, err)
	}

	// Cache it
	d.stores[scope] = store
	return store, nil
}

// ListAllScopes returns pads from all available scopes
func (d *Dispatcher) ListAllScopes() (map[string][]*Pad, []error) {
	// Get base store directory to find all scopes
	baseDir, err := GetStorePath("")
	if err != nil {
		return nil, []error{fmt.Errorf("failed to get base store path: %w", err)}
	}

	// Find all scope directories
	entries, err := os.ReadDir(baseDir)
	if err != nil {
		if os.IsNotExist(err) {
			return make(map[string][]*Pad), nil // Return empty result, no error
		}
		return nil, []error{fmt.Errorf("failed to read store directory: %w", err)}
	}

	results := make(map[string][]*Pad)
	var errors []error

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		scope := entry.Name()
		store, err := d.GetStore(scope)
		if err != nil {
			errors = append(errors, fmt.Errorf("failed to get store for scope %s: %w", scope, err))
			continue
		}

		pads, err := store.List()
		if err != nil {
			errors = append(errors, fmt.Errorf("failed to list pads in scope %s: %w", scope, err))
			continue
		}

		results[scope] = pads
	}

	return results, errors
}

// ParseID parses an ID string into a ScopedID
// Supports both explicit format (scope-id) and implicit format (just id)
func (d *Dispatcher) ParseID(idStr string, currentScope string) (*ScopedID, error) {
	// Check for explicit format (scope-id)
	if strings.Contains(idStr, "-") {
		parts := strings.SplitN(idStr, "-", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("invalid explicit ID format: %s (expected scope-id)", idStr)
		}

		scope := parts[0]
		userIDStr := parts[1]

		userID, err := strconv.Atoi(userIDStr)
		if err != nil {
			return nil, fmt.Errorf("invalid user ID in explicit format: %s", userIDStr)
		}

		if userID <= 0 {
			return nil, fmt.Errorf("user ID must be positive: %d", userID)
		}

		return &ScopedID{
			Scope:  scope,
			UserID: userID,
		}, nil
	}

	// Implicit format - just an integer
	userID, err := strconv.Atoi(idStr)
	if err != nil {
		return nil, fmt.Errorf("invalid ID format: %s (expected integer or scope-id)", idStr)
	}

	if userID <= 0 {
		return nil, fmt.Errorf("user ID must be positive: %d", userID)
	}

	// Resolve scope using precedence rules
	resolvedScope, err := d.ResolveImplicitScope(userID, currentScope)
	if err != nil {
		return nil, fmt.Errorf("failed to resolve scope for ID %d: %w", userID, err)
	}

	return &ScopedID{
		Scope:  resolvedScope,
		UserID: userID,
	}, nil
}

// ResolveImplicitScope resolves which scope to use for an implicit ID
// Precedence: current scope > global scope
func (d *Dispatcher) ResolveImplicitScope(userID int, currentScope string) (string, error) {
	// First, try current scope (highest precedence)
	if currentScope != "" {
		store, err := d.GetStore(currentScope)
		if err == nil {
			if _, _, err := store.Get(userID); err == nil {
				return currentScope, nil
			}
		}
	}

	// Then try global scope
	globalStore, err := d.GetStore("global")
	if err == nil {
		if _, _, err := globalStore.Get(userID); err == nil {
			return "global", nil
		}
	}

	// Finally, check all other scopes
	allResults, _ := d.ListAllScopes()
	for scope := range allResults {
		if scope == currentScope || scope == "global" {
			continue // Already checked
		}

		store, err := d.GetStore(scope)
		if err != nil {
			continue
		}

		if _, _, err := store.Get(userID); err == nil {
			return scope, nil
		}
	}

	return "", fmt.Errorf("pad with ID %d not found in any scope", userID)
}

// GetPad retrieves a pad using ID resolution
func (d *Dispatcher) GetPad(idStr string, currentScope string) (*Pad, string, string, error) {
	scopedID, err := d.ParseID(idStr, currentScope)
	if err != nil {
		return nil, "", "", err
	}

	store, err := d.GetStore(scopedID.Scope)
	if err != nil {
		return nil, "", "", fmt.Errorf("failed to get store for scope %s: %w", scopedID.Scope, err)
	}

	pad, content, err := store.Get(scopedID.UserID)
	if err != nil {
		return nil, "", "", fmt.Errorf("failed to get pad %d from scope %s: %w", scopedID.UserID, scopedID.Scope, err)
	}

	return pad, content, scopedID.Scope, nil
}

// CreatePad creates a pad in the specified scope
func (d *Dispatcher) CreatePad(content, title, scope string) (*Pad, error) {
	store, err := d.GetStore(scope)
	if err != nil {
		return nil, fmt.Errorf("failed to get store for scope %s: %w", scope, err)
	}

	pad, err := store.Create(content, title)
	if err != nil {
		return nil, fmt.Errorf("failed to create pad in scope %s: %w", scope, err)
	}

	return pad, nil
}

// DeletePad deletes a pad using ID resolution
func (d *Dispatcher) DeletePad(idStr string, currentScope string) (string, error) {
	scopedID, err := d.ParseID(idStr, currentScope)
	if err != nil {
		return "", err
	}

	store, err := d.GetStore(scopedID.Scope)
	if err != nil {
		return "", fmt.Errorf("failed to get store for scope %s: %w", scopedID.Scope, err)
	}

	pad, err := store.Delete(scopedID.UserID)
	if err != nil {
		return "", fmt.Errorf("failed to delete pad %d from scope %s: %w", scopedID.UserID, scopedID.Scope, err)
	}

	return FormatExplicitID(scopedID.Scope, pad.UserID), nil
}

// SearchPads searches for pads in a specific scope
func (d *Dispatcher) SearchPads(term, scope string) ([]*SearchResult, error) {
	store, err := d.GetStore(scope)
	if err != nil {
		return nil, fmt.Errorf("failed to get store for scope %s: %w", scope, err)
	}

	results, err := store.Search(term)
	if err != nil {
		return nil, fmt.Errorf("failed to search in scope %s: %w", scope, err)
	}

	return results, nil
}

// SearchAllScopes searches for pads across all available scopes
func (d *Dispatcher) SearchAllScopes(term string) (map[string][]*SearchResult, []error) {
	// Get base store directory to find all scopes
	baseDir, err := GetStorePath("")
	if err != nil {
		return nil, []error{fmt.Errorf("failed to get base store path: %w", err)}
	}

	// Find all scope directories
	entries, err := os.ReadDir(baseDir)
	if err != nil {
		if os.IsNotExist(err) {
			return make(map[string][]*SearchResult), nil
		}
		return nil, []error{fmt.Errorf("failed to read store directory: %w", err)}
	}

	results := make(map[string][]*SearchResult)
	var errors []error

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		scope := entry.Name()
		scopeResults, err := d.SearchPads(term, scope)
		if err != nil {
			errors = append(errors, fmt.Errorf("failed to search scope %s: %w", scope, err))
			continue
		}

		results[scope] = scopeResults
	}

	return results, errors
}

// FormatExplicitID formats a pad's ID in explicit format
func FormatExplicitID(scope string, userID int) string {
	return fmt.Sprintf("%s-%d", scope, userID)
}
