package commands

import (
	"fmt"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// PinMultiple pins multiple scratches by their IDs
func PinMultiple(s *store.Store, global bool, project string, ids []string) ([]string, error) {
	// Resolve all IDs first
	scratches, err := ResolveMultipleIDs(s, global, project, ids)
	if err != nil {
		return nil, err
	}

	// Get current pinned count for the target scope
	currentPinned := s.GetPinnedScratches()
	var scopedPinned []store.Scratch
	for _, scratch := range currentPinned {
		// For global scope, only count global scratches
		// For project scope, only count scratches from that project
		if global && scratch.Project == "global" {
			scopedPinned = append(scopedPinned, scratch)
		} else if !global && scratch.Project == project {
			scopedPinned = append(scopedPinned, scratch)
		}
	}
	currentPinnedCount := len(scopedPinned)

	// Count how many new pins we're trying to add
	newPinsNeeded := 0
	for _, scratch := range scratches {
		if !scratch.IsPinned {
			newPinsNeeded++
		}
	}

	// Check if we'll exceed the limit
	if currentPinnedCount+newPinsNeeded > store.MaxPinnedScratches {
		availableSlots := store.MaxPinnedScratches - currentPinnedCount
		if availableSlots <= 0 {
			return nil, fmt.Errorf("maximum number of pinned scratches (%d) already reached", store.MaxPinnedScratches)
		}
		return nil, fmt.Errorf("cannot pin %d scratches: only %d slots available (max %d)",
			newPinsNeeded, availableSlots, store.MaxPinnedScratches)
	}

	// Collect all scratches to pin with updated pin info
	now := time.Now()
	var pinnedTitles []string
	var scratchesToUpdate []store.Scratch

	for _, scratch := range scratches {
		// Skip already pinned items
		if scratch.IsPinned {
			continue
		}

		scratch.IsPinned = true
		scratch.PinnedAt = now
		scratchesToUpdate = append(scratchesToUpdate, *scratch)
		pinnedTitles = append(pinnedTitles, scratch.Title)
	}

	// Update all scratches atomically
	if len(scratchesToUpdate) > 0 {
		// Get all current scratches
		allScratches := s.GetScratches()

		// Update the scratches that need to be pinned
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
			return nil, fmt.Errorf("failed to pin scratches: %w", err)
		}
	}

	return pinnedTitles, nil
}

// UnpinMultiple unpins multiple scratches by their IDs
func UnpinMultiple(s *store.Store, global bool, project string, ids []string) ([]string, error) {
	// Resolve all IDs first
	scratches, err := ResolveMultipleIDs(s, global, project, ids)
	if err != nil {
		return nil, err
	}

	// Collect all scratches to unpin
	var unpinnedTitles []string
	var scratchesToUpdate []store.Scratch

	for _, scratch := range scratches {
		// Skip non-pinned items
		if !scratch.IsPinned {
			continue
		}

		scratch.IsPinned = false
		scratch.PinnedAt = time.Time{} // Zero value
		scratchesToUpdate = append(scratchesToUpdate, *scratch)
		unpinnedTitles = append(unpinnedTitles, scratch.Title)
	}

	// Update all scratches atomically
	if len(scratchesToUpdate) > 0 {
		// Get all current scratches
		allScratches := s.GetScratches()

		// Update the scratches that need to be unpinned
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
			return nil, fmt.Errorf("failed to unpin scratches: %w", err)
		}
	}

	return unpinnedTitles, nil
}

// Pin marks a scratch as pinned (wrapper for backward compatibility)
func Pin(s *store.Store, global bool, project string, id string) error {
	titles, err := PinMultiple(s, global, project, []string{id})
	if err != nil {
		return err
	}
	if len(titles) == 0 {
		return fmt.Errorf("scratch is already pinned")
	}
	return nil
}

// Unpin removes the pinned status from a scratch (wrapper for backward compatibility)
func Unpin(s *store.Store, global bool, project string, id string) error {
	titles, err := UnpinMultiple(s, global, project, []string{id})
	if err != nil {
		return err
	}
	if len(titles) == 0 {
		return fmt.Errorf("scratch is not pinned")
	}
	return nil
}
