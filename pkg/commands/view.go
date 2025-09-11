package commands

import (
	"fmt"
	"strings"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/store"
)

// ViewMultiple views multiple scratches combined with headers
func ViewMultiple(s *store.Store, all, global bool, project string, ids []string) (string, error) {
	// Use aggregation with headers only for multiple items
	var options AggregateOptions
	if len(ids) > 1 {
		options = AggregateOptionsWithHeaders()
	} else {
		options = DefaultAggregateOptions()
	}

	aggregated, err := AggregateScratchContentsByIDs(s, all, global, project, ids, options)
	if err != nil {
		return "", err
	}

	if len(ids) > 1 {
		return aggregated.GetCombinedContentWithHeaders(), nil
	}
	return aggregated.GetCombinedContent(), nil
}

// View views a single scratch (wrapper for backward compatibility)
func View(s *store.Store, all, global bool, project string, indexStr string) (string, error) {
	return ViewMultiple(s, all, global, project, []string{indexStr})
}

// ViewMultipleWithStoreManager views multiple scratches using StoreManager with scoped ID support
func ViewMultipleWithStoreManager(workingDir string, globalFlag bool, ids []string) (string, error) {
	// Use aggregation with headers only for multiple items
	var options AggregateOptions
	if len(ids) > 1 {
		options = AggregateOptionsWithHeaders()
	} else {
		options = DefaultAggregateOptions()
	}

	aggregated, err := AggregateScratchContentsByIDsWithStoreManager(workingDir, globalFlag, ids, options)
	if err != nil {
		return "", err
	}

	if len(ids) > 1 {
		return aggregated.GetCombinedContentWithHeaders(), nil
	}
	return aggregated.GetCombinedContent(), nil
}

// ViewWithStoreManager views a single scratch using StoreManager (wrapper for backward compatibility)
func ViewWithStoreManager(workingDir string, globalFlag bool, indexStr string) (string, error) {
	return ViewMultipleWithStoreManager(workingDir, globalFlag, []string{indexStr})
}

// AggregateScratchContentsByIDsWithStoreManager resolves IDs (including scoped IDs) and aggregates their content
func AggregateScratchContentsByIDsWithStoreManager(workingDir string, globalFlag bool, ids []string, options AggregateOptions) (*AggregatedContent, error) {
	if len(ids) == 0 {
		return &AggregatedContent{
			Scratches: []*store.Scratch{},
			Contents:  []string{},
			Options:   options,
		}, nil
	}

	// Track unique IDs to handle duplicates
	seen := make(map[string]bool)
	uniqueIDs := make([]string, 0, len(ids))

	// Preserve order but skip duplicates
	for _, id := range ids {
		if id == "" {
			continue
		}
		if !seen[id] {
			seen[id] = true
			uniqueIDs = append(uniqueIDs, id)
		}
	}

	// Create StoreManager
	sm := store.NewStoreManager()

	// Resolve all IDs using StoreManager and track which store they came from
	var scopedScratches []ScopedScratchWithStore
	var errors []string

	for _, id := range uniqueIDs {
		// Resolve the scratch
		scopedScratch, err := ResolveScratchWithStoreManager(workingDir, globalFlag, id)
		if err != nil {
			errors = append(errors, fmt.Sprintf("%s: %v", id, err))
			continue
		}

		// Get the store that contains this scratch
		var scratchStore *store.Store
		if strings.HasPrefix(scopedScratch.Scope, "project:") {
			// Project store
			projectStore, err := sm.GetProjectStore(scopedScratch.Scope, workingDir)
			if err != nil {
				errors = append(errors, fmt.Sprintf("%s: failed to get project store: %v", id, err))
				continue
			}
			scratchStore = projectStore
		} else {
			// Global store
			globalStore, err := sm.GetGlobalStore()
			if err != nil {
				errors = append(errors, fmt.Sprintf("%s: failed to get global store: %v", id, err))
				continue
			}
			scratchStore = globalStore
		}

		scopedScratches = append(scopedScratches, ScopedScratchWithStore{
			Scratch: scopedScratch.Scratch,
			Store:   scratchStore,
			Scope:   scopedScratch.Scope,
		})
	}

	// If any errors occurred, return nil with combined error message
	if len(errors) > 0 {
		return nil, fmt.Errorf("failed to resolve IDs: %s", strings.Join(errors, "; "))
	}

	return AggregateScratchContentsWithStoreManager(sm, workingDir, globalFlag, scopedScratches, options)
}

func readScratchFile(id string) ([]byte, error) {
	fs := config.GetConfig().FileSystem
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		return nil, err
	}
	return fs.ReadFile(path)
}
