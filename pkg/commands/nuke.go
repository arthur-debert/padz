package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
	"time"
)

// NukeResult contains the result of a nuke operation
type NukeResult struct {
	DeletedCount int
	Scope        string
	ProjectName  string
}

// Nuke soft-deletes all scratches in the specified scope
func Nuke(s *store.Store, all bool, project string) (*NukeResult, error) {
	var scratchesToSoftDelete []store.Scratch
	result := &NukeResult{}
	now := time.Now()

	scratches := s.GetScratches()

	// Count only non-deleted scratches for the result
	for _, scratch := range scratches {
		if scratch.IsDeleted {
			continue // Skip already deleted items
		}

		if all {
			// Soft delete all non-deleted scratches across all scopes
			scratchesToSoftDelete = append(scratchesToSoftDelete, scratch)
		} else if project == "" && scratch.Project == "global" {
			// Soft delete only global scratches
			scratchesToSoftDelete = append(scratchesToSoftDelete, scratch)
		} else if project != "" && scratch.Project == project {
			// Soft delete only project-specific scratches
			scratchesToSoftDelete = append(scratchesToSoftDelete, scratch)
		}
	}

	// Set the scope and count
	if all {
		result.Scope = "all"
	} else if project == "" {
		result.Scope = "global"
	} else {
		result.Scope = "project"
		result.ProjectName = project
	}
	result.DeletedCount = len(scratchesToSoftDelete)

	// Soft delete the scratches
	for i := range scratchesToSoftDelete {
		scratchesToSoftDelete[i].IsDeleted = true
		scratchesToSoftDelete[i].DeletedAt = &now
	}

	// Update all scratches with soft-deleted ones
	updated := make([]store.Scratch, len(scratches))
	copy(updated, scratches)

	// Update the scratches that were soft-deleted
	for _, toDelete := range scratchesToSoftDelete {
		for j := range updated {
			if updated[j].ID == toDelete.ID {
				updated[j] = toDelete
				break
			}
		}
	}

	if err := s.SaveScratchesAtomic(updated); err != nil {
		return nil, err
	}

	return result, nil
}
