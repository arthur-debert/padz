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
