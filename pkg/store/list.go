package store

import (
	"sort"
)

// List returns all non-deleted pads sorted by creation time (newest first)
func (s *Store) List() ([]*Pad, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Convert map to slice, excluding deleted items
	pads := make([]*Pad, 0, len(s.metadata.Pads))
	for _, pad := range s.metadata.Pads {
		if !pad.IsDeleted {
			pads = append(pads, pad)
		}
	}

	// Sort by UserID (which is assigned at creation time and stays stable)
	// Higher UserID means newer (since we increment NextID)
	sort.Slice(pads, func(i, j int) bool {
		return pads[i].UserID > pads[j].UserID
	})

	return pads, nil
}

// ListDeleted returns all deleted pads sorted by deletion time (newest first)
func (s *Store) ListDeleted() ([]*Pad, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Convert map to slice, including only deleted items
	pads := make([]*Pad, 0)
	for _, pad := range s.metadata.Pads {
		if pad.IsDeleted {
			pads = append(pads, pad)
		}
	}

	// Sort by deletion time (newest first)
	sort.Slice(pads, func(i, j int) bool {
		if pads[i].DeletedAt == nil || pads[j].DeletedAt == nil {
			return pads[i].UserID > pads[j].UserID
		}
		return pads[i].DeletedAt.After(*pads[j].DeletedAt)
	})

	return pads, nil
}

// ListAll returns all pads including deleted ones
func (s *Store) ListAll() ([]*Pad, error) {
	s.mu.RLock()
	defer s.mu.RUnlock()

	// Convert map to slice
	pads := make([]*Pad, 0, len(s.metadata.Pads))
	for _, pad := range s.metadata.Pads {
		pads = append(pads, pad)
	}

	// Sort by UserID
	sort.Slice(pads, func(i, j int) bool {
		return pads[i].UserID > pads[j].UserID
	})

	return pads, nil
}

// Count returns the number of non-deleted pads in the store
func (s *Store) Count() int {
	s.mu.RLock()
	defer s.mu.RUnlock()

	count := 0
	for _, pad := range s.metadata.Pads {
		if !pad.IsDeleted {
			count++
		}
	}
	return count
}
