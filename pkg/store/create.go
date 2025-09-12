package store

import (
	"fmt"
	"os"
	"path/filepath"
	"time"
)

// Create adds a new pad to the store
func (s *Store) Create(content string, title string) (*Pad, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Generate unique ID and checksum
	id := generateID(content)
	checksum := calculateChecksum(content)

	// Check if content already exists (deduplication)
	for _, pad := range s.metadata.Pads {
		if pad.Checksum == checksum {
			return nil, fmt.Errorf("identical content already exists with ID %d", pad.UserID)
		}
	}

	// Create pad object
	pad := &Pad{
		ID:        id,
		UserID:    s.metadata.NextID,
		Title:     title,
		CreatedAt: time.Now(),
		Size:      int64(len(content)),
		Checksum:  checksum,
	}

	// Save content to file
	contentPath := filepath.Join(s.path, "data", id)
	if err := os.WriteFile(contentPath, []byte(content), 0644); err != nil {
		return nil, fmt.Errorf("failed to write content: %w", err)
	}

	// Update metadata
	s.metadata.Pads[id] = pad
	s.metadata.NextID++

	// Save metadata
	if err := s.saveMetadata(); err != nil {
		// Rollback content file
		_ = os.Remove(contentPath)
		return nil, fmt.Errorf("failed to save metadata: %w", err)
	}

	return pad, nil
}
