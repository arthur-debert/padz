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
		return sortWithPinnedFirst(scratches)
	}

	var filtered []store.Scratch
	for _, scratch := range scratches {
		if global && scratch.Project == "global" {
			filtered = append(filtered, scratch)
		} else if !global && scratch.Project == project {
			filtered = append(filtered, scratch)
		}
	}
	return sortWithPinnedFirst(filtered)
}

// LsWithOriginalIndices returns scratches sorted with pinned first, plus a map of scratch IDs to their original chronological positions
func LsWithOriginalIndices(s *store.Store, all, global bool, project string) ([]store.Scratch, map[string]int) {
	scratches := s.GetScratches()

	// First, get the chronologically sorted list to build the index map
	var chronological []store.Scratch
	if all {
		chronological = sortByCreatedAtDesc(scratches)
	} else {
		var filtered []store.Scratch
		for _, scratch := range scratches {
			if global && scratch.Project == "global" {
				filtered = append(filtered, scratch)
			} else if !global && scratch.Project == project {
				filtered = append(filtered, scratch)
			}
		}
		chronological = sortByCreatedAtDesc(filtered)
	}

	// Build map of ID to chronological position
	originalIndices := make(map[string]int)
	for i, scratch := range chronological {
		originalIndices[scratch.ID] = i + 1
	}

	// Return the pinned-first sorted list and the index map
	return Ls(s, all, global, project), originalIndices
}

func sortByCreatedAtDesc(scratches []store.Scratch) []store.Scratch {
	sorted := make([]store.Scratch, len(scratches))
	copy(sorted, scratches)
	sort.Slice(sorted, func(i, j int) bool {
		return sorted[i].CreatedAt.After(sorted[j].CreatedAt)
	})
	return sorted
}

func sortWithPinnedFirst(scratches []store.Scratch) []store.Scratch {
	sorted := make([]store.Scratch, len(scratches))
	copy(sorted, scratches)
	sort.Slice(sorted, func(i, j int) bool {
		// Pinned items come first
		if sorted[i].IsPinned != sorted[j].IsPinned {
			return sorted[i].IsPinned
		}
		// Among pinned items, sort by PinnedAt (newest first)
		if sorted[i].IsPinned && sorted[j].IsPinned {
			return sorted[i].PinnedAt.After(sorted[j].PinnedAt)
		}
		// Among non-pinned items, sort by CreatedAt (newest first)
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
