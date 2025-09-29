package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
	"time"
)

// CleanupOptions defines options for cleanup operation
type CleanupOptions struct {
	DaysForActive  int // Days to keep active scratches (default 30)
	DaysForDeleted int // Days to keep soft-deleted scratches (default 7)
}

func Cleanup(s *store.Store, days int) error {
	// Use default options for backward compatibility
	opts := CleanupOptions{
		DaysForActive:  days,
		DaysForDeleted: 7, // Default to 7 days for soft-deleted items
	}
	return CleanupWithOptions(s, opts)
}

// CleanupWithOptions performs cleanup with configurable options
func CleanupWithOptions(s *store.Store, opts CleanupOptions) error {
	scratches := s.GetAllScratches()
	activeCutoff := time.Now().AddDate(0, 0, -opts.DaysForActive)
	deletedCutoff := time.Now().AddDate(0, 0, -opts.DaysForDeleted)

	var scratchesToKeep []store.Scratch
	var scratchesToPermanentlyDelete []store.Scratch

	for _, scratch := range scratches {
		if scratch.IsDeleted {
			// Handle soft-deleted items
			if scratch.DeletedAt != nil && scratch.DeletedAt.Before(deletedCutoff) {
				// Permanently delete old soft-deleted items
				scratchesToPermanentlyDelete = append(scratchesToPermanentlyDelete, scratch)
			} else {
				// Keep recently soft-deleted items
				scratchesToKeep = append(scratchesToKeep, scratch)
			}
		} else {
			// Handle active items
			if scratch.CreatedAt.Before(activeCutoff) {
				// Old active items get permanently deleted
				scratchesToPermanentlyDelete = append(scratchesToPermanentlyDelete, scratch)
			} else {
				// Keep recent active items
				scratchesToKeep = append(scratchesToKeep, scratch)
			}
		}
	}

	// Permanently delete files
	for _, scratch := range scratchesToPermanentlyDelete {
		if err := PermanentlyDeleteScratchFile(scratch.ID); err != nil {
			// Continue with other files even if one fails
			continue
		}
	}

	return s.SaveScratchesAtomic(scratchesToKeep)
}
