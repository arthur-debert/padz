package commands

import (
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
