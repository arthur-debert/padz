package store

import (
	"fmt"
	"os"
	"strconv"
	"strings"
)

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
func (d *Dispatcher) ListAllScopes(showDeleted bool, includeDeleted bool, showPinned bool) (map[string][]*Pad, []error) {
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

		// List pads based on flags
		var pads []*Pad
		if showDeleted {
			pads, err = store.ListDeleted()
		} else if showPinned {
			pads, err = store.ListPinned()
		} else if includeDeleted {
			pads, err = store.ListAll()
		} else {
			pads, err = store.List()
		}
		if err != nil {
			errors = append(errors, fmt.Errorf("failed to list pads in scope %s: %w", scope, err))
			continue
		}

		results[scope] = pads
	}

	return results, errors
}

// IDType represents the type of ID (normal, deleted, pinned)
type IDType int

const (
	IDTypeNormal IDType = iota
	IDTypeDeleted
	IDTypePinned
)

// ParsedID represents a fully parsed ID with type and scope
type ParsedID struct {
	Type   IDType
	Scope  string
	UserID int
}

// ParseID parses an ID string into a ParsedID
// Supports:
// - Normal IDs: "1", "2", "3"
// - Deleted IDs: "d1", "d2", "d3"
// - Pinned IDs: "p1", "p2", "p3" (for future use)
// - Explicit format: "scope-1", "global-d1", etc.
func (d *Dispatcher) ParseID(idStr string, currentScope string) (*ParsedID, error) {
	var idType = IDTypeNormal
	var scope string
	var userIDStr string

	// Check for explicit format (scope-id)
	if strings.Contains(idStr, "-") {
		parts := strings.SplitN(idStr, "-", 2)
		if len(parts) != 2 {
			return nil, fmt.Errorf("invalid explicit ID format: %s (expected scope-id)", idStr)
		}

		scope = parts[0]
		userIDStr = parts[1]
	} else {
		// Implicit format - check for prefix
		if strings.HasPrefix(idStr, "d") {
			idType = IDTypeDeleted
			userIDStr = idStr[1:]
		} else if strings.HasPrefix(idStr, "p") {
			idType = IDTypePinned
			userIDStr = idStr[1:]
		} else {
			userIDStr = idStr
		}
	}

	// Parse the numeric ID
	userID, err := strconv.Atoi(userIDStr)
	if err != nil {
		return nil, fmt.Errorf("invalid ID format: %s (expected integer or prefix+integer)", idStr)
	}

	if userID <= 0 {
		return nil, fmt.Errorf("user ID must be positive: %d", userID)
	}

	// For explicit format, we already have the scope
	if scope != "" {
		return &ParsedID{
			Type:   idType,
			Scope:  scope,
			UserID: userID,
		}, nil
	}

	// For implicit format, resolve scope
	resolvedScope, err := d.ResolveImplicitScope(userID, currentScope, idType)
	if err != nil {
		return nil, fmt.Errorf("failed to resolve scope for ID %s: %w", idStr, err)
	}

	return &ParsedID{
		Type:   idType,
		Scope:  resolvedScope,
		UserID: userID,
	}, nil
}

// ResolveImplicitScope resolves which scope to use for an implicit ID
// Precedence: current scope > global scope
func (d *Dispatcher) ResolveImplicitScope(userID int, currentScope string, idType IDType) (string, error) {
	// Choose the right getter based on ID type
	var getFunc func(*Store, int) (*Pad, string, error)
	switch idType {
	case IDTypeDeleted:
		getFunc = (*Store).GetDeleted
	case IDTypePinned:
		getFunc = (*Store).GetPinned
	default:
		getFunc = (*Store).Get
	}

	// First, try current scope (highest precedence)
	if currentScope != "" {
		store, err := d.GetStore(currentScope)
		if err == nil {
			if _, _, err := getFunc(store, userID); err == nil {
				return currentScope, nil
			}
		}
	}

	// Then try global scope
	globalStore, err := d.GetStore("global")
	if err == nil {
		if _, _, err := getFunc(globalStore, userID); err == nil {
			return "global", nil
		}
	}

	// Finally, check all other scopes
	allResults, _ := d.ListAllScopes(false, false, false)
	for scope := range allResults {
		if scope == currentScope || scope == "global" {
			continue // Already checked
		}

		store, err := d.GetStore(scope)
		if err != nil {
			continue
		}

		if _, _, err := getFunc(store, userID); err == nil {
			return scope, nil
		}
	}

	var itemType string
	switch idType {
	case IDTypeDeleted:
		itemType = "deleted pad"
	case IDTypePinned:
		itemType = "pinned pad"
	default:
		itemType = "pad"
	}
	return "", fmt.Errorf("%s with ID %d not found in any scope", itemType, userID)
}

// GetPad retrieves a pad using ID resolution
func (d *Dispatcher) GetPad(idStr string, currentScope string) (*Pad, string, string, error) {
	parsedID, err := d.ParseID(idStr, currentScope)
	if err != nil {
		return nil, "", "", err
	}

	store, err := d.GetStore(parsedID.Scope)
	if err != nil {
		return nil, "", "", fmt.Errorf("failed to get store for scope %s: %w", parsedID.Scope, err)
	}

	// Choose the right getter based on ID type
	var pad *Pad
	var content string
	switch parsedID.Type {
	case IDTypeDeleted:
		pad, content, err = store.GetDeleted(parsedID.UserID)
	case IDTypePinned:
		pad, content, err = store.GetPinned(parsedID.UserID)
	default:
		pad, content, err = store.Get(parsedID.UserID)
	}

	if err != nil {
		return nil, "", "", fmt.Errorf("failed to get pad %d from scope %s: %w", parsedID.UserID, parsedID.Scope, err)
	}

	return pad, content, parsedID.Scope, nil
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
	parsedID, err := d.ParseID(idStr, currentScope)
	if err != nil {
		return "", err
	}

	store, err := d.GetStore(parsedID.Scope)
	if err != nil {
		return "", fmt.Errorf("failed to get store for scope %s: %w", parsedID.Scope, err)
	}

	// For deleted items, we can't delete them again
	if parsedID.Type == IDTypeDeleted {
		return "", fmt.Errorf("item is already deleted")
	}

	// For pinned items, we can't delete them by pinned ID
	if parsedID.Type == IDTypePinned {
		return "", fmt.Errorf("cannot delete item using pinned ID (use regular ID)")
	}

	pad, err := store.Delete(parsedID.UserID)
	if err != nil {
		return "", fmt.Errorf("failed to delete pad %d from scope %s: %w", parsedID.UserID, parsedID.Scope, err)
	}

	return FormatExplicitID(parsedID.Scope, pad.UserID), nil
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
