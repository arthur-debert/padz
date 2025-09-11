package store2

import (
	"sort"
)

// List returns all pads sorted by creation time (newest first)
func (s *Store) List() ([]*Pad, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Convert map to slice
	pads := make([]*Pad, 0, len(s.metadata.Pads))
	for _, pad := range s.metadata.Pads {
		pads = append(pads, pad)
	}

	// Sort by creation time, newest first
	sort.Slice(pads, func(i, j int) bool {
		return pads[i].CreatedAt.After(pads[j].CreatedAt)
	})

	// Re-index user IDs based on sort order
	// This ensures the newest pad is always ID 1
	for i, pad := range pads {
		pad.UserID = i + 1
	}

	return pads, nil
}

// Count returns the number of pads in the store
func (s *Store) Count() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.metadata.Pads)
}
