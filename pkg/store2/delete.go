package store2

import (
	"fmt"
	"os"
	"path/filepath"
)

// Delete removes a pad from the store
func (s *Store) Delete(userID int) (*Pad, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Find pad by user ID
	var targetPad *Pad
	var targetID string
	for id, pad := range s.metadata.Pads {
		if pad.UserID == userID {
			targetPad = pad
			targetID = id
			break
		}
	}

	if targetPad == nil {
		return nil, fmt.Errorf("pad with ID %d not found", userID)
	}

	// Remove content file
	contentPath := filepath.Join(s.path, "data", targetID)
	_ = os.Remove(contentPath) // Ignore errors - metadata cleanup is more important

	// Remove from metadata
	delete(s.metadata.Pads, targetID)

	// Save metadata
	if err := s.saveMetadata(); err != nil {
		return nil, fmt.Errorf("failed to save metadata after delete: %w", err)
	}

	return targetPad, nil
}
