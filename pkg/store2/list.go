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

	// Sort by UserID (which is assigned at creation time and stays stable)
	// Higher UserID means newer (since we increment NextID)
	sort.Slice(pads, func(i, j int) bool {
		return pads[i].UserID > pads[j].UserID
	})

	return pads, nil
}

// Count returns the number of pads in the store
func (s *Store) Count() int {
	s.mu.RLock()
	defer s.mu.RUnlock()
	return len(s.metadata.Pads)
}
