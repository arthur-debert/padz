package store

import (
	"fmt"
	"os"
	"path/filepath"
	"time"
)

// Update modifies an existing pad with new content and/or title
func (s *Store) Update(userID int, content, title string) (*Pad, error) {
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

	// Calculate new checksum and size
	newChecksum := calculateChecksum(content)
	newSize := int64(len(content))

	// Update pad metadata
	targetPad.Title = title
	targetPad.Size = newSize
	targetPad.Checksum = newChecksum
	s.metadata.UpdatedAt = time.Now()

	// Save content to file
	contentPath := filepath.Join(s.path, "data", targetPad.ID)
	if err := os.WriteFile(contentPath, []byte(content), 0644); err != nil {
		return nil, fmt.Errorf("failed to write content: %w", err)
	}

	// Save metadata
	if err := s.saveMetadata(); err != nil {
		return nil, fmt.Errorf("failed to save metadata: %w", err)
	}

	// Return copy of updated pad
	updatedPad := *targetPad
	return &updatedPad, nil
}
