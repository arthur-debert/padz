package store

import (
	"fmt"
	"os"
	"path/filepath"
)

// Restore undeletes a soft-deleted pad
func (s *Store) Restore(userID int) (*Pad, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Find deleted pad by user ID
	var targetPad *Pad
	for _, pad := range s.metadata.Pads {
		if pad.UserID == userID && pad.IsDeleted {
			targetPad = pad
			break
		}
	}

	if targetPad == nil {
		return nil, fmt.Errorf("deleted pad with ID %d not found", userID)
	}

	// Restore it
	targetPad.IsDeleted = false
	targetPad.DeletedAt = nil

	// Save metadata
	if err := s.saveMetadata(); err != nil {
		return nil, fmt.Errorf("failed to save metadata after restore: %w", err)
	}

	return targetPad, nil
}

// Flush permanently deletes a soft-deleted pad
func (s *Store) Flush(userID int) (*Pad, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Find deleted pad by user ID
	var targetPad *Pad
	var targetID string
	for id, pad := range s.metadata.Pads {
		if pad.UserID == userID && pad.IsDeleted {
			targetPad = pad
			targetID = id
			break
		}
	}

	if targetPad == nil {
		return nil, fmt.Errorf("deleted pad with ID %d not found", userID)
	}

	// Remove content file
	contentPath := filepath.Join(s.path, "data", targetID)
	_ = os.Remove(contentPath) // Ignore errors

	// Remove from metadata
	delete(s.metadata.Pads, targetID)

	// Save metadata
	if err := s.saveMetadata(); err != nil {
		return nil, fmt.Errorf("failed to save metadata after flush: %w", err)
	}

	return targetPad, nil
}

// FlushAll permanently deletes all soft-deleted pads
func (s *Store) FlushAll() (int, error) {
	s.mu.Lock()
	defer s.mu.Unlock()

	// Find all deleted pads
	var toDelete []string
	for id, pad := range s.metadata.Pads {
		if pad.IsDeleted {
			toDelete = append(toDelete, id)
		}
	}

	// Remove content files and metadata entries
	for _, id := range toDelete {
		contentPath := filepath.Join(s.path, "data", id)
		_ = os.Remove(contentPath)
		delete(s.metadata.Pads, id)
	}

	// Save metadata
	if err := s.saveMetadata(); err != nil {
		return 0, fmt.Errorf("failed to save metadata after flush all: %w", err)
	}

	return len(toDelete), nil
}
