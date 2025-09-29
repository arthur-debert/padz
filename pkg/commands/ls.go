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

func Ls(s *store.Store, global bool, project string) []store.Scratch {
	// Default behavior - exclude deleted items
	return LsWithMode(s, global, project, ListModeActive)
}

// LsWithMode lists scratches with delete filtering options
func LsWithMode(s *store.Store, global bool, project string, mode ListMode) []store.Scratch {
	var scratches []store.Scratch

	switch mode {
	case ListModeActive:
		// Use store-level filtering for active items
		scratches = s.GetScratchesWithFilter(project, global)
	case ListModeDeleted:
		// Use store-level filtering for deleted items
		scratches = s.GetDeletedScratchesWithFilter(project, global)
	case ListModeAll:
		// Use store-level filtering for all items
		scratches = s.GetAllScratchesWithFilter(project, global)
	}

	// For ListModeAll, we need special sorting by most recent activity
	// (nanostore can't do this in SQL since it requires CASE statement)
	if mode == ListModeAll {
		return sortByMostRecentActivity(scratches)
	}

	// For other modes, nanostore already handles sorting via OrderBy
	return scratches
}

// sortByMostRecentActivity sorts scratches by their most recent activity
// (creation date for active items, deletion date for deleted items)
func sortByMostRecentActivity(scratches []store.Scratch) []store.Scratch {
	sorted := make([]store.Scratch, len(scratches))
	copy(sorted, scratches)
	sort.Slice(sorted, func(i, j int) bool {
		// Get the most recent activity date for each scratch
		iTime := sorted[i].CreatedAt
		if sorted[i].IsDeleted && sorted[i].DeletedAt != nil {
			iTime = *sorted[i].DeletedAt
		}

		jTime := sorted[j].CreatedAt
		if sorted[j].IsDeleted && sorted[j].DeletedAt != nil {
			jTime = *sorted[j].DeletedAt
		}

		return iTime.After(jTime)
	})
	return sorted
}

func GetScratchByIndex(s *store.Store, global bool, project string, indexStr string) (*store.Scratch, error) {
	// Check if it's a deleted index (d1, d2, etc)
	if len(indexStr) > 1 && indexStr[0] == 'd' {
		deletedIndexStr := indexStr[1:]
		deletedIndex, err := strconv.Atoi(deletedIndexStr)
		if err != nil {
			return nil, fmt.Errorf("invalid deleted index: %s", indexStr)
		}

		// Get deleted items already sorted by deletion time (newest first) by nanostore
		deletedScratches := s.GetDeletedScratchesWithFilter(project, global)

		if deletedIndex < 1 || deletedIndex > len(deletedScratches) {
			return nil, fmt.Errorf("deleted index out of range: %s", indexStr)
		}

		return &deletedScratches[deletedIndex-1], nil
	}

	// Get active scratches for other cases
	scratches := Ls(s, global, project)

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
func ResolveScratchID(s *store.Store, global bool, project string, id string) (*store.Scratch, error) {
	// Check if it's a deleted index (d1, d2, etc) - padz convention, not nanostore
	if len(id) > 1 && id[0] == 'd' {
		return GetScratchByIndex(s, global, project, id)
	}

	// Try to resolve the ID using nanostore's ResolveUUID
	// This handles SimpleIDs (1, 2, p1, etc.) and full UUIDs
	uuid, err := s.ResolveIDToUUID(id)
	if err != nil {
		// If it can't be resolved, it might be a partial UUID prefix
		// Try to find a scratch with matching UUID prefix
		scratches := s.GetAllScratchesWithFilter(project, global)
		for i := range scratches {
			if strings.HasPrefix(scratches[i].ID, id) {
				return &scratches[i], nil
			}
		}
		return nil, fmt.Errorf("scratch not found: %s", id)
	}

	// Get the scratch by its UUID
	scratch, err := s.GetScratchByUUID(uuid)
	if err != nil {
		return nil, err
	}

	// Verify it belongs to the correct project/scope
	if global && scratch.Project != "global" {
		return nil, fmt.Errorf("scratch not found in global scope: %s", id)
	}
	if !global && project != "" && scratch.Project != project {
		return nil, fmt.Errorf("scratch not found in project %s: %s", project, id)
	}

	return scratch, nil
}
