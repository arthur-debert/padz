package commands

import (
	"fmt"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// Flush performs hard deletion of soft-deleted scratches
func Flush(s *store.Store, all bool, global bool, project string, indexStr string, olderThan time.Duration) error {
	// If a specific index is provided, flush that single scratch
	if indexStr != "" {
		scratch, err := GetScratchByIndex(s, all, global, project, indexStr)
		if err != nil {
			return err
		}

		if !scratch.IsDeleted {
			return fmt.Errorf("scratch %s is not deleted", indexStr)
		}

		if err := PermanentlyDeleteScratchFile(scratch.ID); err != nil {
			return err
		}

		return s.RemoveScratchAtomic(scratch.ID)
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
