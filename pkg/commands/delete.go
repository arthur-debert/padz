package commands

import (
	"fmt"
	"strings"
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/store"
)

// DeleteMultiple soft deletes multiple scratches by their IDs
func DeleteMultiple(s *store.Store, all bool, global bool, project string, ids []string) ([]string, error) {
	// Resolve all IDs first
	scratches, err := ResolveMultipleIDs(s, all, global, project, ids)
	if err != nil {
		return nil, err
	}

	// Collect all scratches to delete with updated deletion info
	now := time.Now()
	var deletedTitles []string
	var scratchesToUpdate []store.Scratch

	for _, scratch := range scratches {
		// Skip already deleted items
		if scratch.IsDeleted {
			continue
		}

		scratch.IsDeleted = true
		scratch.DeletedAt = &now
		scratchesToUpdate = append(scratchesToUpdate, *scratch)
		deletedTitles = append(deletedTitles, scratch.Title)
	}

	// Update all scratches atomically
	if len(scratchesToUpdate) > 0 {
		// Get all current scratches
		allScratches := s.GetScratches()

		// Update the scratches that need to be deleted
		for i := range allScratches {
			for _, toUpdate := range scratchesToUpdate {
				if allScratches[i].ID == toUpdate.ID {
					allScratches[i] = toUpdate
					break
				}
			}
		}

		// Save all scratches back
		if err := s.SaveScratchesAtomic(allScratches); err != nil {
			return nil, fmt.Errorf("failed to delete scratches: %w", err)
		}
	}

	return deletedTitles, nil
}

// Delete soft deletes a single scratch (wrapper for backward compatibility)
func Delete(s *store.Store, all bool, global bool, project string, indexStr string) error {
	titles, err := DeleteMultiple(s, all, global, project, []string{indexStr})
	if err != nil {
		return err
	}
	if len(titles) == 0 {
		return fmt.Errorf("scratch already deleted or not found")
	}
	return nil
}

// DeleteMultipleWithStoreManager soft deletes multiple scratches by their IDs using StoreManager with scoped ID support
func DeleteMultipleWithStoreManager(workingDir string, globalFlag bool, ids []string) ([]string, error) {
	if len(ids) == 0 {
		return []string{}, nil
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

	// Group scratches by their store scope for efficient operations
	type storeUpdate struct {
		store     *store.Store
		scratches []*store.Scratch
	}
	storeUpdates := make(map[string]*storeUpdate)

	// Resolve all IDs and group by store
	var errors []string

	for _, id := range uniqueIDs {
		scopedScratch, err := ResolveScratchWithStoreManager(workingDir, globalFlag, id)
		if err != nil {
			errors = append(errors, fmt.Sprintf("%s: %v", id, err))
			continue
		}

		// Skip already deleted items
		if scopedScratch.IsDeleted {
			continue
		}

		// Get the store for this scope
		sm := store.NewStoreManager()
		var currentStore *store.Store
		if scopedScratch.Scope == "global" {
			currentStore, err = sm.GetGlobalStore()
		} else {
			currentStore, err = sm.GetProjectStore(scopedScratch.Scope, workingDir)
		}
		if err != nil {
			errors = append(errors, fmt.Sprintf("%s: failed to get store: %v", id, err))
			continue
		}

		// Group by store scope
		storeKey := scopedScratch.Scope
		if storeUpdates[storeKey] == nil {
			storeUpdates[storeKey] = &storeUpdate{
				store:     currentStore,
				scratches: []*store.Scratch{},
			}
		}

		storeUpdates[storeKey].scratches = append(storeUpdates[storeKey].scratches, scopedScratch.Scratch)
	}

	// If any errors occurred, return nil with combined error message
	if len(errors) > 0 {
		return nil, fmt.Errorf("failed to resolve IDs: %s", strings.Join(errors, "; "))
	}

	// Perform deletions for each store
	now := time.Now()
	var deletedTitles []string

	for _, update := range storeUpdates {
		// Mark scratches as deleted
		var scratchesToUpdate []store.Scratch
		for _, scratch := range update.scratches {
			scratch.IsDeleted = true
			scratch.DeletedAt = &now
			scratchesToUpdate = append(scratchesToUpdate, *scratch)
			deletedTitles = append(deletedTitles, scratch.Title)
		}

		// Update store atomically
		if len(scratchesToUpdate) > 0 {
			// Get all current scratches from this store
			allScratches := update.store.GetScratches()

			// Update the scratches that need to be deleted
			for i := range allScratches {
				for _, toUpdate := range scratchesToUpdate {
					if allScratches[i].ID == toUpdate.ID {
						allScratches[i] = toUpdate
						break
					}
				}
			}

			// Save all scratches back to this store
			if err := update.store.SaveScratchesAtomic(allScratches); err != nil {
				return nil, fmt.Errorf("failed to delete scratches in scope: %w", err)
			}
		}
	}

	return deletedTitles, nil
}

// DeleteWithStoreManager soft deletes a single scratch using StoreManager (wrapper for backward compatibility)
func DeleteWithStoreManager(workingDir string, globalFlag bool, indexStr string) error {
	titles, err := DeleteMultipleWithStoreManager(workingDir, globalFlag, []string{indexStr})
	if err != nil {
		return err
	}
	if len(titles) == 0 {
		return fmt.Errorf("scratch already deleted or not found")
	}
	return nil
}

// PermanentlyDeleteScratchFile removes the physical file from disk
// This is used by the flush command for hard deletion
func PermanentlyDeleteScratchFile(id string) error {
	fs := config.GetConfig().FileSystem
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		return err
	}
	return fs.Remove(path)
}
