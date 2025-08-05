package commands

import (
	"fmt"
	"sort"
	"strconv"
	"github.com/arthur-debert/padz/pkg/store"
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

	index, err := strconv.Atoi(indexStr)
	if err != nil {
		return nil, fmt.Errorf("invalid index: %s", indexStr)
	}

	if index < 1 || index > len(scratches) {
		return nil, fmt.Errorf("index out of range: %d", index)
	}

	return &scratches[index-1], nil
}
