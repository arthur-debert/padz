package store2

import (
	"fmt"
	"os"
	"path/filepath"
	"strconv"
)

// Get retrieves a pad by user ID
func (s *Store) Get(userID int) (*Pad, string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Find pad by user ID
	for _, pad := range s.metadata.Pads {
		if pad.UserID == userID {
			// Load content
			content, err := s.loadContent(pad.ID)
			if err != nil {
				return nil, "", err
			}
			return pad, content, nil
		}
	}

	return nil, "", fmt.Errorf("pad with ID %d not found", userID)
}

// GetByID retrieves a pad by its internal ID
func (s *Store) GetByID(id string) (*Pad, string, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	pad, exists := s.metadata.Pads[id]
	if !exists {
		return nil, "", fmt.Errorf("pad with ID %s not found", id)
	}

	// Load content
	content, err := s.loadContent(pad.ID)
	if err != nil {
		return nil, "", err
	}

	return pad, content, nil
}

// ParseID parses a user-provided ID which could be an integer or explicit scope ID
func (s *Store) ParseID(idStr string) (int, error) {
	// For now, just parse as integer
	// Later, dispatcher will handle scope prefixes
	userID, err := strconv.Atoi(idStr)
	if err != nil {
		return 0, fmt.Errorf("invalid ID format: %s", idStr)
	}
	return userID, nil
}

// loadContent loads content from disk
func (s *Store) loadContent(id string) (string, error) {
	contentPath := filepath.Join(s.path, "data", id)
	data, err := os.ReadFile(contentPath)
	if err != nil {
		return "", fmt.Errorf("failed to read content: %w", err)
	}
	return string(data), nil
}
