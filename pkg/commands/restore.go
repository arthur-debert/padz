package commands

import (
	"fmt"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// RestoreMultiple restores multiple soft-deleted scratches by their IDs
func RestoreMultiple(s *store.Store, global bool, project string, ids []string) ([]string, error) {
	// Resolve all IDs first
	scratches, err := s.ResolveBulkIDs(ids, project, global)
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

	// Restore scratches using atomic bulk operation
	if len(scratchesToRestore) > 0 {
		uuids := make([]string, 0, len(scratchesToRestore))
		for _, scratch := range scratchesToRestore {
			uuid, err := s.ResolveIDToUUID(scratch.ID)
			if err != nil {
				return nil, fmt.Errorf("failed to resolve UUID for %s: %w", scratch.ID, err)
			}
			uuids = append(uuids, uuid)
		}

		updates := &store.TypedScratch{
			Activity:  "active",
			DeletedAt: "", // Clear deletion timestamp (empty string)
		}

		_, err := s.UpdateByUUIDs(uuids, updates)
		if err != nil {
			return nil, fmt.Errorf("failed to restore scratches: %w", err)
		}
	}

	return restoredTitles, nil
}

// Restore restores soft-deleted scratches
func Restore(s *store.Store, global bool, project string, indexStr string, newerThan time.Duration) error {
	// If a specific index is provided, restore that single scratch using RestoreMultiple
	if indexStr != "" {
		_, err := RestoreMultiple(s, global, project, []string{indexStr})
		return err
	}

	// Otherwise, restore multiple scratches based on criteria
	// Get deleted scratches that match criteria
	var deletedScratches []store.Scratch
	if global {
		deletedScratches = s.GetDeletedScratchesWithFilter("", true)
	} else {
		deletedScratches = s.GetDeletedScratchesWithFilter(project, false)
	}

	var toRestore []store.Scratch
	cutoffTime := time.Now().Add(-newerThan)

	for _, scratch := range deletedScratches {
		// Check if new enough to restore (if newerThan is specified)
		if newerThan > 0 && scratch.DeletedAt != nil && scratch.DeletedAt.Before(cutoffTime) {
			continue
		}
		toRestore = append(toRestore, scratch)
	}

	// Restore scratches using atomic bulk operation
	if len(toRestore) > 0 {
		uuids := make([]string, 0, len(toRestore))
		for _, scratch := range toRestore {
			uuid, err := s.ResolveIDToUUID(scratch.ID)
			if err != nil {
				return fmt.Errorf("failed to resolve UUID for %s: %w", scratch.ID, err)
			}
			uuids = append(uuids, uuid)
		}

		updates := &store.TypedScratch{
			Activity:  "active",
			DeletedAt: "", // Clear deletion timestamp (empty string)
		}

		_, err := s.UpdateByUUIDs(uuids, updates)
		if err != nil {
			return fmt.Errorf("failed to restore scratches: %w", err)
		}
	}

	return nil
}

// RestoreAll restores all soft-deleted scratches
func RestoreAll(s *store.Store) error {
	return Restore(s, false, "", "", 0)
}
