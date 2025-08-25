package commands

import (
	"fmt"
	"github.com/arthur-debert/padz/pkg/store"
	"sort"
	"strconv"
	"strings"
)

func Ls(s *store.Store, all, global bool, project string) []store.Scratch {
	scratches := s.GetScratches()
	if all {
		return sortByCreatedAtDesc(scratches)
	}

	var filtered []store.Scratch
	for _, scratch := range scratches {
		if global && scratch.Project == "global" {
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
	scratches := Ls(s, all, global, project)

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

	// Regular index
	index, err := strconv.Atoi(indexStr)
	if err != nil {
		return nil, fmt.Errorf("invalid index: %s", indexStr)
	}

	if index < 1 || index > len(scratches) {
		return nil, fmt.Errorf("index out of range: %d", index)
	}

	return &scratches[index-1], nil
}

// ResolveScratchID resolves various ID formats (index, pinned index, hash) to a scratch
func ResolveScratchID(s *store.Store, all, global bool, project string, id string) (*store.Scratch, error) {
	// Try as index (including pinned index)
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
