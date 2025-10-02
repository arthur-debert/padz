package commands

import (
	"fmt"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// DeleteMultiple soft deletes multiple scratches by their IDs using atomic bulk operations
func DeleteMultiple(s *store.Store, global bool, project string, ids []string) ([]string, error) {
	// Resolve all IDs to UUIDs first to prevent ID instability
	scratches, err := ResolveMultipleIDs(s, global, project, ids)
	if err != nil {
		return nil, err
	}

	// Extract UUIDs and titles for bulk operation
	uuids := make([]string, 0, len(scratches))
	deletedTitles := make([]string, 0, len(scratches))

	for _, scratch := range scratches {
		// Skip already deleted items
		if scratch.IsDeleted {
			continue
		}

		// Resolve SimpleID to UUID for bulk operation
		uuid, err := s.ResolveIDToUUID(scratch.ID)
		if err != nil {
			return nil, fmt.Errorf("failed to resolve UUID for %s: %w", scratch.ID, err)
		}

		uuids = append(uuids, uuid)
		deletedTitles = append(deletedTitles, scratch.Title)
	}

	// Perform atomic bulk deletion using nanostore's new UpdateByUUIDs
	if len(uuids) > 0 {
		now := time.Now()
		updates := &store.TypedScratch{
			Activity:  "deleted",
			DeletedAt: now.Format(time.RFC3339),
		}

		_, err := s.UpdateByUUIDs(uuids, updates)
		if err != nil {
			return nil, fmt.Errorf("failed to delete scratches: %w", err)
		}
	}

	return deletedTitles, nil
}

// Delete soft deletes a single scratch (wrapper for backward compatibility)
func Delete(s *store.Store, global bool, project string, indexStr string) error {
	titles, err := DeleteMultiple(s, global, project, []string{indexStr})
	if err != nil {
		return err
	}
	if len(titles) == 0 {
		return fmt.Errorf("scratch already deleted or not found")
	}
	return nil
}

// PermanentlyDeleteScratchFile is a no-op since content is now stored in the scratch itself
// This is used by the flush command for hard deletion
func PermanentlyDeleteScratchFile(id string) error {
	// Content is stored in the scratch itself, no files to delete
	return nil
}
