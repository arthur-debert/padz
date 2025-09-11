package commands

import (
	"fmt"
	"strings"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// RestoreMultiple restores multiple soft-deleted scratches by their IDs
func RestoreMultiple(s *store.Store, all bool, global bool, project string, ids []string) ([]string, error) {
	// Resolve all IDs first
	scratches, err := ResolveMultipleIDs(s, all, global, project, ids)
	if err != nil {
		return nil, err
	}

	// Collect all scratches to restore
	var scratchesToRestore []store.Scratch
	var restoredTitles []string

	for _, scratch := range scratches {
		// Only restore items that are actually deleted
		if !scratch.IsDeleted {
			continue
		}

		scratch.IsDeleted = false
		scratch.DeletedAt = nil
		scratchesToRestore = append(scratchesToRestore, *scratch)
		restoredTitles = append(restoredTitles, scratch.Title)
	}

	// Update all scratches atomically
	if len(scratchesToRestore) > 0 {
		// Get all current scratches
		allScratches := s.GetScratches()

		// Update the scratches that need to be restored
		for i := range allScratches {
			for _, toRestore := range scratchesToRestore {
				if allScratches[i].ID == toRestore.ID {
					allScratches[i] = toRestore
					break
				}
			}
		}

		// Save atomically
		if err := s.SaveScratchesAtomic(allScratches); err != nil {
			return nil, err
		}
	}

	return restoredTitles, nil
}

// Restore restores soft-deleted scratches
func Restore(s *store.Store, all bool, global bool, project string, indexStr string, newerThan time.Duration) error {
	// If a specific index is provided, restore that single scratch using RestoreMultiple
	if indexStr != "" {
		_, err := RestoreMultiple(s, all, global, project, []string{indexStr})
		return err
	}

	// Otherwise, restore multiple scratches based on criteria
	scratches := s.GetScratches()
	var toRestore []store.Scratch
	cutoffTime := time.Now().Add(-newerThan)

	for i, scratch := range scratches {
		if !scratch.IsDeleted {
			continue
		}

		// Filter by project/global if not all
		if !all {
			if global && scratch.Project != "global" {
				continue
			}
			if !global && scratch.Project != project {
				continue
			}
		}

		// Check if new enough to restore (if newerThan is specified)
		if newerThan > 0 && scratch.DeletedAt != nil && scratch.DeletedAt.Before(cutoffTime) {
			continue
		}

		toRestore = append(toRestore, scratches[i])
	}

	// Restore scratches
	for i := range toRestore {
		toRestore[i].IsDeleted = false
		toRestore[i].DeletedAt = nil
	}

	// Update all scratches with restored ones
	updated := make([]store.Scratch, len(scratches))
	copy(updated, scratches)

	for _, restored := range toRestore {
		for j := range updated {
			if updated[j].ID == restored.ID {
				updated[j] = restored
				break
			}
		}
	}

	return s.SaveScratchesAtomic(updated)
}

// RestoreAll restores all soft-deleted scratches
func RestoreAll(s *store.Store) error {
	return Restore(s, true, false, "", "", 0)
}

// RestoreMultipleWithStoreManager restores multiple soft-deleted scratches using StoreManager approach
func RestoreMultipleWithStoreManager(workingDir string, globalFlag bool, ids []string) ([]string, error) {
	if len(ids) == 0 {
		return []string{}, nil
	}

	// Track unique IDs to handle duplicates
	seen := make(map[string]bool)
	uniqueIDs := make([]string, 0, len(ids))
	for _, id := range ids {
		if id == "" {
			continue
		}
		if !seen[id] {
			seen[id] = true
			uniqueIDs = append(uniqueIDs, id)
		}
	}

	// Group operations by store for efficiency
	type storeUpdate struct {
		store     *store.Store
		scratches []*store.Scratch
	}
	storeUpdates := make(map[string]*storeUpdate)

	// Resolve all IDs and group by store
	var errors []string
	sm := store.NewStoreManager()

	for _, id := range uniqueIDs {
		scopedScratch, err := ResolveScratchWithStoreManager(workingDir, globalFlag, id)
		if err != nil {
			errors = append(errors, fmt.Sprintf("%s: %v", id, err))
			continue
		}

		// Skip non-deleted items
		if !scopedScratch.IsDeleted {
			continue
		}

		// Get the store for this scope
		var currentStore *store.Store
		if strings.HasPrefix(scopedScratch.Scope, "project:") {
			currentStore, err = sm.GetProjectStore(scopedScratch.Scope, workingDir)
			if err != nil {
				errors = append(errors, fmt.Sprintf("%s: failed to get project store: %v", id, err))
				continue
			}
		} else {
			currentStore, err = sm.GetGlobalStore()
			if err != nil {
				errors = append(errors, fmt.Sprintf("%s: failed to get global store: %v", id, err))
				continue
			}
		}

		storeKey := scopedScratch.Scope
		if _, exists := storeUpdates[storeKey]; !exists {
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

	// Perform restore for each store
	var restoredTitles []string

	for _, update := range storeUpdates {
		// Get all current scratches from this store
		allScratches := update.store.GetScratches()

		// Update the scratches that need to be restored
		for i := range allScratches {
			for _, toRestore := range update.scratches {
				if allScratches[i].ID == toRestore.ID && allScratches[i].IsDeleted {
					allScratches[i].IsDeleted = false
					allScratches[i].DeletedAt = nil
					restoredTitles = append(restoredTitles, allScratches[i].Title)
				}
			}
		}

		// Save all scratches back to this store
		if err := update.store.SaveScratchesAtomic(allScratches); err != nil {
			return nil, fmt.Errorf("failed to restore scratches in store %s: %w", update.store.GetBasePath(), err)
		}
	}

	return restoredTitles, nil
}

// RestoreWithStoreManager restores soft-deleted scratches based on criteria using StoreManager approach
func RestoreWithStoreManager(workingDir string, globalFlag bool, newerThan time.Duration) ([]string, error) {
	sm := store.NewStoreManager()

	// Determine which store to use
	currentStore, scope, err := sm.GetCurrentStore(workingDir, globalFlag)
	if err != nil {
		return nil, fmt.Errorf("failed to get current store: %w", err)
	}

	// Get all scratches and find ones to restore
	scratches := currentStore.GetScratches()
	var toRestore []store.Scratch
	var restoredTitles []string
	cutoffTime := time.Now().Add(-newerThan)

	for i, scratch := range scratches {
		if !scratch.IsDeleted {
			continue
		}

		// Check if new enough to restore (if newerThan is specified)
		if newerThan > 0 && scratch.DeletedAt != nil && scratch.DeletedAt.Before(cutoffTime) {
			continue
		}

		scratches[i].IsDeleted = false
		scratches[i].DeletedAt = nil
		toRestore = append(toRestore, scratches[i])
		restoredTitles = append(restoredTitles, scratches[i].Title)
	}

	// Save atomically if there are changes
	if len(toRestore) > 0 {
		if err := currentStore.SaveScratchesAtomic(scratches); err != nil {
			return nil, fmt.Errorf("failed to restore scratches in %s: %w", scope, err)
		}
	}

	return restoredTitles, nil
}
