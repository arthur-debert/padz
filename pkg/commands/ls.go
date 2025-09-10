package commands

import (
	"fmt"
	"github.com/arthur-debert/padz/pkg/store"
	"sort"
	"strconv"
	"strings"
)

// ListMode defines how to filter deleted items
type ListMode string

const (
	ListModeActive  ListMode = "active"  // Only non-deleted items (default)
	ListModeDeleted ListMode = "deleted" // Only deleted items
	ListModeAll     ListMode = "all"     // Both deleted and non-deleted items
)

func Ls(s *store.Store, all, global bool, project string) []store.Scratch {
	// Default behavior - exclude deleted items
	return LsWithMode(s, all, global, project, ListModeActive)
}

// LsWithMode lists scratches with delete filtering options
func LsWithMode(s *store.Store, all, global bool, project string, mode ListMode) []store.Scratch {
	scratches := s.GetScratches()

	var filtered []store.Scratch
	for _, scratch := range scratches {
		// Filter by deletion status based on mode
		switch mode {
		case ListModeActive:
			if scratch.IsDeleted {
				continue
			}
		case ListModeDeleted:
			if !scratch.IsDeleted {
				continue
			}
		case ListModeAll:
			// Include everything
		}

		// Filter by project/global
		if all {
			filtered = append(filtered, scratch)
		} else if global && scratch.Project == "global" {
			filtered = append(filtered, scratch)
		} else if !global && scratch.Project == project {
			filtered = append(filtered, scratch)
		}
	}

	return sortByCreatedAtDesc(filtered)
}

func sortByCreatedAtDesc(scratches []store.Scratch) []store.Scratch {
	sorted := make([]store.Scratch, len(scratches))
	copy(sorted, scratches)
	sort.Slice(sorted, func(i, j int) bool {
		return sorted[i].CreatedAt.After(sorted[j].CreatedAt)
	})
	return sorted
}

func GetScratchByIndex(s *store.Store, all, global bool, project string, indexStr string) (*store.Scratch, error) {
	// When looking for deleted items (d1, d2, etc), we need to get all items including deleted
	var scratches []store.Scratch
	if len(indexStr) > 1 && indexStr[0] == 'd' {
		scratches = LsWithMode(s, all, global, project, ListModeAll)
	} else {
		scratches = Ls(s, all, global, project)
	}

	// Check if it's a deleted index (d1, d2, etc)
	if len(indexStr) > 1 && indexStr[0] == 'd' {
		deletedIndexStr := indexStr[1:]
		deletedIndex, err := strconv.Atoi(deletedIndexStr)
		if err != nil {
			return nil, fmt.Errorf("invalid deleted index: %s", indexStr)
		}

		// Get deleted items sorted by deletion time (newest first)
		var deletedScratches []store.Scratch
		for _, scratch := range scratches {
			if scratch.IsDeleted {
				deletedScratches = append(deletedScratches, scratch)
			}
		}

		// Sort by DeletedAt descending (newest first)
		sort.Slice(deletedScratches, func(i, j int) bool {
			if deletedScratches[i].DeletedAt == nil || deletedScratches[j].DeletedAt == nil {
				return false
			}
			return deletedScratches[i].DeletedAt.After(*deletedScratches[j].DeletedAt)
		})

		if deletedIndex < 1 || deletedIndex > len(deletedScratches) {
			return nil, fmt.Errorf("deleted index out of range: %s", indexStr)
		}

		return &deletedScratches[deletedIndex-1], nil
	}

	// Check if it's a pinned index (p1, p2, etc)
	if len(indexStr) > 1 && indexStr[0] == 'p' {
		pinnedIndexStr := indexStr[1:]
		pinnedIndex, err := strconv.Atoi(pinnedIndexStr)
		if err != nil {
			return nil, fmt.Errorf("invalid pinned index: %s", indexStr)
		}

		// Count pinned items
		pinnedCount := 0
		for i, scratch := range scratches {
			if scratch.IsPinned {
				pinnedCount++
				if pinnedCount == pinnedIndex {
					return &scratches[i], nil
				}
			}
		}
		return nil, fmt.Errorf("pinned index out of range: %s", indexStr)
	}

	// Regular index - exclude deleted items
	var activeScratches []store.Scratch
	for _, scratch := range scratches {
		if !scratch.IsDeleted {
			activeScratches = append(activeScratches, scratch)
		}
	}

	index, err := strconv.Atoi(indexStr)
	if err != nil {
		return nil, fmt.Errorf("invalid index: %s", indexStr)
	}

	if index < 1 || index > len(activeScratches) {
		return nil, fmt.Errorf("index out of range: %d", index)
	}

	return &activeScratches[index-1], nil
}

// ResolveScratchID resolves various ID formats (index, pinned index, deleted index, hash) to a scratch
func ResolveScratchID(s *store.Store, all, global bool, project string, id string) (*store.Scratch, error) {
	// Try as index (including pinned index and deleted index)
	scratch, err := GetScratchByIndex(s, all, global, project, id)
	if err == nil {
		return scratch, nil
	}

	// Try as hash ID
	scratches := s.GetScratches()
	for i := range scratches {
		if strings.HasPrefix(scratches[i].ID, id) {
			return &scratches[i], nil
		}
	}

	return nil, fmt.Errorf("scratch not found: %s", id)
}
