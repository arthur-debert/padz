package store

import (
	"fmt"
	"time"
)

// Delete soft deletes a pad from the store
func (s *Store) Delete(userID int) (*Pad, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Find pad by user ID (excluding already deleted)
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

	// Mark as deleted
	now := time.Now()
	targetPad.IsDeleted = true
	targetPad.DeletedAt = &now

	// Save metadata
	if err := s.saveMetadata(); err != nil {
		return nil, fmt.Errorf("failed to save metadata after delete: %w", err)
	}

	return targetPad, nil
}
