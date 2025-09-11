package store2

import (
	"fmt"
	"os"
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
