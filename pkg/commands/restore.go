package commands

import (
	"fmt"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// Restore restores soft-deleted scratches
func Restore(s *store.Store, all bool, global bool, project string, indexStr string, newerThan time.Duration) error {
	// If a specific index is provided, restore that single scratch
	if indexStr != "" {
		scratch, err := GetScratchByIndex(s, all, global, project, indexStr)
		if err != nil {
			return err
		}

		if !scratch.IsDeleted {
			return fmt.Errorf("scratch %s is not deleted", indexStr)
		}

		scratch.IsDeleted = false
		scratch.DeletedAt = nil

		return s.UpdateScratchAtomic(*scratch)
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
