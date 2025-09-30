package commands

import (
	"fmt"
	"sort"

	"github.com/arthur-debert/padz/pkg/store"
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
	// Delegate to store's comprehensive ID resolution
	scratches, err := s.ResolveBulkIDs([]string{indexStr}, project, global)
	if err != nil {
		return nil, err
	}
	if len(scratches) == 0 {
		return nil, fmt.Errorf("scratch not found: %s", indexStr)
	}
	return scratches[0], nil
}

// ResolveScratchID resolves various ID formats (index, pinned index, deleted index, hash) to a scratch
func ResolveScratchID(s *store.Store, global bool, project string, id string) (*store.Scratch, error) {
	// Delegate to store's comprehensive ID resolution
	scratches, err := s.ResolveBulkIDs([]string{id}, project, global)
	if err != nil {
		return nil, err
	}
	if len(scratches) == 0 {
		return nil, fmt.Errorf("scratch not found: %s", id)
	}
	return scratches[0], nil
}
