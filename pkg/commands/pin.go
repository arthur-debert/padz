package commands

import (
	"fmt"
	"strings"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// PinMultiple pins multiple scratches by their IDs
func PinMultiple(s *store.Store, all, global bool, project string, ids []string) ([]string, error) {
	// Resolve all IDs first
	scratches, err := ResolveMultipleIDs(s, all, global, project, ids)
	if err != nil {
		return nil, err
	}

	// Get current pinned count
	currentPinned := s.GetPinnedScratches()
	currentPinnedCount := len(currentPinned)

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
func UnpinMultiple(s *store.Store, all, global bool, project string, ids []string) ([]string, error) {
	// Resolve all IDs first
	scratches, err := ResolveMultipleIDs(s, all, global, project, ids)
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
func Pin(s *store.Store, all, global bool, project string, id string) error {
	titles, err := PinMultiple(s, all, global, project, []string{id})
	if err != nil {
		return err
	}
	if len(titles) == 0 {
		return fmt.Errorf("scratch is already pinned")
	}
	return nil
}

// Unpin removes the pinned status from a scratch (wrapper for backward compatibility)
func Unpin(s *store.Store, all, global bool, project string, id string) error {
	titles, err := UnpinMultiple(s, all, global, project, []string{id})
	if err != nil {
		return err
	}
	if len(titles) == 0 {
		return fmt.Errorf("scratch is not pinned")
	}
	return nil
}

// PinMultipleWithStoreManager pins multiple scratches using StoreManager approach
func PinMultipleWithStoreManager(workingDir string, globalFlag bool, ids []string) ([]string, error) {
	if len(ids) == 0 {
		return []string{}, nil
	}

	// Track unique IDs to handle duplicates
	seen := make(map[string]bool)
	uniqueIDs := make([]string, 0, len(ids))
	for _, id := range ids {
		if id == "" {
			continue
		}
		if !seen[id] {
			seen[id] = true
			uniqueIDs = append(uniqueIDs, id)
		}
	}

	// Group operations by store for efficiency
	type storeUpdate struct {
		store     *store.Store
		scratches []*store.Scratch
	}
	storeUpdates := make(map[string]*storeUpdate)

	// Resolve all IDs and group by store
	var errors []string
	sm := store.NewStoreManager()

	for _, id := range uniqueIDs {
		scopedScratch, err := ResolveScratchWithStoreManager(workingDir, globalFlag, id)
		if err != nil {
			errors = append(errors, fmt.Sprintf("%s: %v", id, err))
			continue
		}

		// Skip already pinned items
		if scopedScratch.IsPinned {
			continue
		}

		// Get the store for this scope
		var currentStore *store.Store
		if strings.HasPrefix(scopedScratch.Scope, "project:") {
			currentStore, err = sm.GetProjectStore(scopedScratch.Scope, workingDir)
			if err != nil {
				errors = append(errors, fmt.Sprintf("%s: failed to get project store: %v", id, err))
				continue
			}
		} else {
			currentStore, err = sm.GetGlobalStore()
			if err != nil {
				errors = append(errors, fmt.Sprintf("%s: failed to get global store: %v", id, err))
				continue
			}
		}

		storeKey := scopedScratch.Scope
		if _, exists := storeUpdates[storeKey]; !exists {
			storeUpdates[storeKey] = &storeUpdate{
				store:     currentStore,
				scratches: []*store.Scratch{},
			}
		}

		storeUpdates[storeKey].scratches = append(storeUpdates[storeKey].scratches, scopedScratch.Scratch)
	}

	// If any errors occurred, return nil with combined error message
	if len(errors) > 0 {
		return nil, fmt.Errorf("failed to resolve IDs: %s", strings.Join(errors, "; "))
	}

	// Check total pinned count across all stores
	totalCurrentPinned := 0
	for _, update := range storeUpdates {
		currentPinned := update.store.GetPinnedScratches()
		totalCurrentPinned += len(currentPinned)
	}

	// Count new pins needed
	newPinsNeeded := 0
	for _, update := range storeUpdates {
		for _, scratch := range update.scratches {
			if !scratch.IsPinned {
				newPinsNeeded++
			}
		}
	}

	// Check if we'll exceed the limit
	if totalCurrentPinned+newPinsNeeded > store.MaxPinnedScratches {
		availableSlots := store.MaxPinnedScratches - totalCurrentPinned
		if availableSlots <= 0 {
			return nil, fmt.Errorf("maximum number of pinned scratches (%d) already reached", store.MaxPinnedScratches)
		}
		return nil, fmt.Errorf("cannot pin %d scratches: only %d slots available (max %d)",
			newPinsNeeded, availableSlots, store.MaxPinnedScratches)
	}

	// Perform pinning for each store
	now := time.Now()
	var pinnedTitles []string

	for _, update := range storeUpdates {
		// Get all current scratches from this store
		allScratches := update.store.GetScratches()

		// Update the scratches that need to be pinned
		for i := range allScratches {
			for _, toPin := range update.scratches {
				if allScratches[i].ID == toPin.ID && !allScratches[i].IsPinned {
					allScratches[i].IsPinned = true
					allScratches[i].PinnedAt = now
					pinnedTitles = append(pinnedTitles, allScratches[i].Title)
				}
			}
		}

		// Save all scratches back to this store
		if err := update.store.SaveScratchesAtomic(allScratches); err != nil {
			return nil, fmt.Errorf("failed to pin scratches in store %s: %w", update.store.GetBasePath(), err)
		}
	}

	return pinnedTitles, nil
}

// UnpinMultipleWithStoreManager unpins multiple scratches using StoreManager approach
func UnpinMultipleWithStoreManager(workingDir string, globalFlag bool, ids []string) ([]string, error) {
	if len(ids) == 0 {
		return []string{}, nil
	}

	// Track unique IDs to handle duplicates
	seen := make(map[string]bool)
	uniqueIDs := make([]string, 0, len(ids))
	for _, id := range ids {
		if id == "" {
			continue
		}
		if !seen[id] {
			seen[id] = true
			uniqueIDs = append(uniqueIDs, id)
		}
	}

	// Group operations by store for efficiency
	type storeUpdate struct {
		store     *store.Store
		scratches []*store.Scratch
	}
	storeUpdates := make(map[string]*storeUpdate)

	// Resolve all IDs and group by store
	var errors []string
	sm := store.NewStoreManager()

	for _, id := range uniqueIDs {
		scopedScratch, err := ResolveScratchWithStoreManager(workingDir, globalFlag, id)
		if err != nil {
			errors = append(errors, fmt.Sprintf("%s: %v", id, err))
			continue
		}

		// Skip non-pinned items
		if !scopedScratch.IsPinned {
			continue
		}

		// Get the store for this scope
		var currentStore *store.Store
		if strings.HasPrefix(scopedScratch.Scope, "project:") {
			currentStore, err = sm.GetProjectStore(scopedScratch.Scope, workingDir)
			if err != nil {
				errors = append(errors, fmt.Sprintf("%s: failed to get project store: %v", id, err))
				continue
			}
		} else {
			currentStore, err = sm.GetGlobalStore()
			if err != nil {
				errors = append(errors, fmt.Sprintf("%s: failed to get global store: %v", id, err))
				continue
			}
		}

		storeKey := scopedScratch.Scope
		if _, exists := storeUpdates[storeKey]; !exists {
			storeUpdates[storeKey] = &storeUpdate{
				store:     currentStore,
				scratches: []*store.Scratch{},
			}
		}

		storeUpdates[storeKey].scratches = append(storeUpdates[storeKey].scratches, scopedScratch.Scratch)
	}

	// If any errors occurred, return nil with combined error message
	if len(errors) > 0 {
		return nil, fmt.Errorf("failed to resolve IDs: %s", strings.Join(errors, "; "))
	}

	// Perform unpinning for each store
	var unpinnedTitles []string

	for _, update := range storeUpdates {
		// Get all current scratches from this store
		allScratches := update.store.GetScratches()

		// Update the scratches that need to be unpinned
		for i := range allScratches {
			for _, toUnpin := range update.scratches {
				if allScratches[i].ID == toUnpin.ID && allScratches[i].IsPinned {
					allScratches[i].IsPinned = false
					allScratches[i].PinnedAt = time.Time{} // Zero value
					unpinnedTitles = append(unpinnedTitles, allScratches[i].Title)
				}
			}
		}

		// Save all scratches back to this store
		if err := update.store.SaveScratchesAtomic(allScratches); err != nil {
			return nil, fmt.Errorf("failed to unpin scratches in store %s: %w", update.store.GetBasePath(), err)
		}
	}

	return unpinnedTitles, nil
}
