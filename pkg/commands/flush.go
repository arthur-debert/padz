package commands

import (
	"fmt"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// FlushMultiple performs hard deletion of specific soft-deleted scratches by their IDs
func FlushMultiple(s *store.Store, all bool, global bool, project string, ids []string) (int, error) {
	// Resolve all IDs first
	scratches, err := ResolveMultipleIDs(s, all, global, project, ids)
	if err != nil {
		return 0, err
	}

	// Validate that all provided IDs are actually deleted items
	var toFlush []store.Scratch
	for _, scratch := range scratches {
		if !scratch.IsDeleted {
			return 0, fmt.Errorf("scratch %s is not deleted", scratch.ID)
		}
		toFlush = append(toFlush, *scratch)
	}

	// Permanently delete files
	var deletedCount int
	for _, scratch := range toFlush {
		if err := PermanentlyDeleteScratchFile(scratch.ID); err != nil {
			// Continue with other files even if one fails
			continue
		}
		deletedCount++
	}

	// Remove from metadata
	allScratches := s.GetScratches()
	remaining := make([]store.Scratch, 0)
	flushedIDs := make(map[string]bool)
	for _, scratch := range toFlush {
		flushedIDs[scratch.ID] = true
	}

	for _, scratch := range allScratches {
		if !flushedIDs[scratch.ID] {
			remaining = append(remaining, scratch)
		}
	}

	if err := s.SaveScratchesAtomic(remaining); err != nil {
		return deletedCount, err
	}

	return deletedCount, nil
}

// Flush performs hard deletion of soft-deleted scratches
func Flush(s *store.Store, all bool, global bool, project string, indexStr string, olderThan time.Duration) error {
	// If a specific index is provided, flush that single scratch using FlushMultiple
	if indexStr != "" {
		_, err := FlushMultiple(s, all, global, project, []string{indexStr})
		return err
	}

	// Otherwise, flush multiple scratches based on criteria
	scratches := s.GetScratches()
	var toFlush []store.Scratch
	cutoffTime := time.Now().Add(-olderThan)

	for _, scratch := range scratches {
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

		// Check if old enough to flush (if olderThan is specified)
		if olderThan > 0 && scratch.DeletedAt != nil && scratch.DeletedAt.After(cutoffTime) {
			continue
		}

		toFlush = append(toFlush, scratch)
	}

	// Permanently delete files
	for _, scratch := range toFlush {
		if err := PermanentlyDeleteScratchFile(scratch.ID); err != nil {
			// Continue with other files even if one fails
			continue
		}
	}

	// Remove from metadata
	remaining := make([]store.Scratch, 0)
	flushedIDs := make(map[string]bool)
	for _, scratch := range toFlush {
		flushedIDs[scratch.ID] = true
	}

	for _, scratch := range scratches {
		if !flushedIDs[scratch.ID] {
			remaining = append(remaining, scratch)
		}
	}

	return s.SaveScratchesAtomic(remaining)
}

// FlushAll flushes all soft-deleted scratches
func FlushAll(s *store.Store) error {
	return Flush(s, true, false, "", "", 0)
}

// FlushMultipleWithStoreManager performs hard deletion of specific soft-deleted scratches using StoreManager
func FlushMultipleWithStoreManager(workingDir string, globalFlag bool, ids []string) (int, error) {
	sm := store.NewStoreManager()

	// Group IDs by store
	type storeFlushInfo struct {
		store     *store.Store
		scope     string
		scratches []*store.Scratch
	}
	storeMap := make(map[string]*storeFlushInfo)

	// Resolve all IDs and group by store
	for _, id := range ids {
		// Resolve the scratch using StoreManager
		scopedScratch, err := ResolveScratchWithStoreManager(workingDir, globalFlag, id)
		if err != nil {
			return 0, err
		}

		// Verify it's deleted
		if !scopedScratch.IsDeleted {
			return 0, fmt.Errorf("scratch %s is not deleted", id)
		}

		// Get the appropriate store
		var targetStore *store.Store
		if scopedScratch.Scope == "global" {
			targetStore, err = sm.GetGlobalStore()
		} else {
			targetStore, err = sm.GetProjectStore(scopedScratch.Scope, workingDir)
		}
		if err != nil {
			return 0, err
		}

		// Group by store
		if storeMap[scopedScratch.Scope] == nil {
			storeMap[scopedScratch.Scope] = &storeFlushInfo{
				store:     targetStore,
				scope:     scopedScratch.Scope,
				scratches: []*store.Scratch{},
			}
		}
		storeMap[scopedScratch.Scope].scratches = append(storeMap[scopedScratch.Scope].scratches, scopedScratch.Scratch)
	}

	// Flush from each store
	totalFlushed := 0
	for _, info := range storeMap {
		// Permanently delete files
		for _, scratch := range info.scratches {
			if err := PermanentlyDeleteScratchFile(scratch.ID); err != nil {
				// Continue with other files even if one fails
				continue
			}
			totalFlushed++
		}

		// Remove from metadata
		allScratches := info.store.GetScratches()
		remaining := make([]store.Scratch, 0)
		flushedIDs := make(map[string]bool)
		for _, scratch := range info.scratches {
			flushedIDs[scratch.ID] = true
		}

		for _, scratch := range allScratches {
			if !flushedIDs[scratch.ID] {
				remaining = append(remaining, scratch)
			}
		}

		if err := info.store.SaveScratchesAtomic(remaining); err != nil {
			return totalFlushed, err
		}
	}

	return totalFlushed, nil
}

// FlushWithStoreManager performs hard deletion of soft-deleted scratches using StoreManager
func FlushWithStoreManager(workingDir string, globalFlag bool, allFlag bool, olderThan time.Duration) error {
	sm := store.NewStoreManager()

	// Determine which stores to flush
	storesToFlush := make(map[string]*store.Store)

	if allFlag {
		// Flush from global store
		globalStore, err := sm.GetGlobalStore()
		if err != nil {
			return fmt.Errorf("failed to get global store: %w", err)
		}
		storesToFlush["global"] = globalStore

		// Also flush from current project store if available
		currentStore, scope, err := sm.GetCurrentStore(workingDir, false)
		if err == nil && scope != "global" {
			storesToFlush[scope] = currentStore
		}
	} else {
		// Flush only from the current store (based on global flag)
		currentStore, scope, err := sm.GetCurrentStore(workingDir, globalFlag)
		if err != nil {
			return fmt.Errorf("failed to get current store: %w", err)
		}
		storesToFlush[scope] = currentStore
	}

	// Flush from each store
	for _, storeInstance := range storesToFlush {
		scratches := storeInstance.GetScratches()
		var toFlush []store.Scratch
		cutoffTime := time.Now().Add(-olderThan)

		for _, scratch := range scratches {
			if !scratch.IsDeleted {
				continue
			}

			// Check if old enough to flush (if olderThan is specified)
			if olderThan > 0 && scratch.DeletedAt != nil && scratch.DeletedAt.After(cutoffTime) {
				continue
			}

			toFlush = append(toFlush, scratch)
		}

		// Permanently delete files
		for _, scratch := range toFlush {
			if err := PermanentlyDeleteScratchFile(scratch.ID); err != nil {
				// Continue with other files even if one fails
				continue
			}
		}

		// Remove from metadata
		remaining := make([]store.Scratch, 0)
		flushedIDs := make(map[string]bool)
		for _, scratch := range toFlush {
			flushedIDs[scratch.ID] = true
		}

		for _, scratch := range scratches {
			if !flushedIDs[scratch.ID] {
				remaining = append(remaining, scratch)
			}
		}

		if err := storeInstance.SaveScratchesAtomic(remaining); err != nil {
			return err
		}
	}

	return nil
}
