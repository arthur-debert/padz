package commands

import (
	"fmt"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// FlushMultiple performs hard deletion of specific soft-deleted scratches by their IDs
func FlushMultiple(s *store.Store, global bool, project string, ids []string) (int, error) {
	// Resolve all IDs first
	scratches, err := ResolveMultipleIDs(s, global, project, ids)
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

	// Hard delete from store using atomic bulk operation
	uuids := make([]string, 0, len(toFlush))
	for _, scratch := range toFlush {
		uuid, err := s.ResolveIDToUUID(scratch.ID)
		if err != nil {
			return deletedCount, fmt.Errorf("failed to resolve UUID for %s: %w", scratch.ID, err)
		}
		uuids = append(uuids, uuid)
	}

	if len(uuids) > 0 {
		_, err := s.DeleteByUUIDs(uuids)
		if err != nil {
			return deletedCount, fmt.Errorf("failed to flush scratches: %w", err)
		}
	}

	return deletedCount, nil
}

// Flush performs hard deletion of soft-deleted scratches
func Flush(s *store.Store, global bool, project string, indexStr string, olderThan time.Duration) error {
	// If a specific index is provided, flush that single scratch using FlushMultiple
	if indexStr != "" {
		_, err := FlushMultiple(s, global, project, []string{indexStr})
		return err
	}

	// Otherwise, flush multiple scratches based on criteria
	// Get deleted scratches that match criteria
	var deletedScratches []store.Scratch
	if global {
		deletedScratches = s.GetDeletedScratchesWithFilter("", true)
	} else {
		deletedScratches = s.GetDeletedScratchesWithFilter(project, false)
	}

	var toFlush []store.Scratch
	cutoffTime := time.Now().Add(-olderThan)

	for _, scratch := range deletedScratches {
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

	// Hard delete from store using atomic bulk operation
	uuids := make([]string, 0, len(toFlush))
	for _, scratch := range toFlush {
		uuid, err := s.ResolveIDToUUID(scratch.ID)
		if err != nil {
			return fmt.Errorf("failed to resolve UUID for %s: %w", scratch.ID, err)
		}
		uuids = append(uuids, uuid)
	}

	if len(uuids) > 0 {
		_, err := s.DeleteByUUIDs(uuids)
		if err != nil {
			return fmt.Errorf("failed to flush scratches: %w", err)
		}
	}

	return nil
}

// FlushAll flushes all soft-deleted scratches
func FlushAll(s *store.Store) error {
	return Flush(s, false, "", "", 0)
}
