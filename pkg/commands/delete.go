package commands

import (
	"fmt"
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
