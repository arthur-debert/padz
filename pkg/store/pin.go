package store

import (
	"fmt"
	"time"
)

// Pin marks a pad as pinned
func (s *Store) Pin(userID int) (*Pad, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Find pad by user ID (excluding deleted)
	var targetPad *Pad
	for _, pad := range s.metadata.Pads {
		if pad.UserID == userID && !pad.IsDeleted {
			targetPad = pad
			break
		}
	}

	if targetPad == nil {
		return nil, fmt.Errorf("pad with ID %d not found", userID)
	}

	if targetPad.IsPinned {
		return nil, fmt.Errorf("pad with ID %d is already pinned", userID)
	}

	// Pin it
	now := time.Now()
	targetPad.IsPinned = true
	targetPad.PinnedAt = &now

	// Save metadata
	if err := s.saveMetadata(); err != nil {
		return nil, fmt.Errorf("failed to save metadata after pin: %w", err)
	}

	return targetPad, nil
}

// Unpin removes pin status from a pad
func (s *Store) Unpin(userID int) (*Pad, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Find pad by user ID (excluding deleted)
	var targetPad *Pad
	for _, pad := range s.metadata.Pads {
		if pad.UserID == userID && !pad.IsDeleted {
			targetPad = pad
			break
		}
	}

	if targetPad == nil {
		return nil, fmt.Errorf("pad with ID %d not found", userID)
	}

	if !targetPad.IsPinned {
		return nil, fmt.Errorf("pad with ID %d is not pinned", userID)
	}

	// Unpin it
	targetPad.IsPinned = false
	targetPad.PinnedAt = nil

	// Save metadata
	if err := s.saveMetadata(); err != nil {
		return nil, fmt.Errorf("failed to save metadata after unpin: %w", err)
	}

	return targetPad, nil
}

// GetPinned retrieves a pinned pad by user ID
func (s *Store) GetPinned(userID int) (*Pad, string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Find pad by user ID (only pinned, not deleted)
	for _, pad := range s.metadata.Pads {
		if pad.UserID == userID && pad.IsPinned && !pad.IsDeleted {
			// Load content
			content, err := s.loadContent(pad.ID)
			if err != nil {
				return nil, "", err
			}
			return pad, content, nil
		}
	}

	return nil, "", fmt.Errorf("pinned pad with ID %d not found", userID)
}

// ListPinned returns all pinned pads sorted by pin time (newest first)
func (s *Store) ListPinned() ([]*Pad, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Convert map to slice, including only pinned, non-deleted items
	pads := make([]*Pad, 0)
	for _, pad := range s.metadata.Pads {
		if pad.IsPinned && !pad.IsDeleted {
			pads = append(pads, pad)
		}
	}

	// Sort by pin time (newest first)
	if len(pads) > 1 {
		for i := 0; i < len(pads)-1; i++ {
			for j := i + 1; j < len(pads); j++ {
				iPinnedAt := pads[i].PinnedAt
				jPinnedAt := pads[j].PinnedAt

				// Handle nil pinned times (shouldn't happen but be safe)
				if iPinnedAt == nil && jPinnedAt != nil {
					pads[i], pads[j] = pads[j], pads[i]
				} else if iPinnedAt != nil && jPinnedAt != nil && jPinnedAt.After(*iPinnedAt) {
					pads[i], pads[j] = pads[j], pads[i]
				}
			}
		}
	}

	return pads, nil
}
