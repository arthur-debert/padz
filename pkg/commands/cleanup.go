package commands

import (
	"fmt"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
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
	scratches := s.GetScratches()
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

// CleanupWithStoreManager performs cleanup on all stores using StoreManager approach
func CleanupWithStoreManager(workingDir string, days int) (int, error) {
	sm := store.NewStoreManager()
	totalDeleted := 0

	// Use default options for backward compatibility
	opts := CleanupOptions{
		DaysForActive:  days,
		DaysForDeleted: 7, // Default to 7 days for soft-deleted items
	}

	// Clean up global store
	globalStore, err := sm.GetGlobalStore()
	if err != nil {
		return 0, fmt.Errorf("failed to get global store: %w", err)
	}

	deleted, err := cleanupSingleStore(globalStore, opts)
	if err != nil {
		return totalDeleted, fmt.Errorf("failed to cleanup global store: %w", err)
	}
	totalDeleted += deleted

	// For project stores, we need to find all project stores
	// Since cleanup affects all projects, we'll iterate through known projects
	// This is a limitation - we can only clean up project stores that we know about
	// In the future, we might want to scan for all .padz directories

	// Try to clean up current project if we're in one
	currentStore, _, err := sm.GetCurrentStore(workingDir, false)
	if err == nil && currentStore != globalStore { // Make sure it's not the global store
		deleted, err = cleanupSingleStore(currentStore, opts)
		if err != nil {
			// Log but continue
			log.Error().Err(err).Msg("Failed to cleanup current project store")
		} else {
			totalDeleted += deleted
		}
	}

	return totalDeleted, nil
}

// cleanupSingleStore performs cleanup on a single store
func cleanupSingleStore(s *store.Store, opts CleanupOptions) (int, error) {
	scratches := s.GetScratches()
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

	// Save the remaining scratches
	if err := s.SaveScratchesAtomic(scratchesToKeep); err != nil {
		return 0, err
	}

	return len(scratchesToPermanentlyDelete), nil
}
