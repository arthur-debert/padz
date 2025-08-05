package commands

import (
	"fmt"
	"github.com/arthur-debert/padz/pkg/store"
	"os"
)

// NukeResult contains the result of a nuke operation
type NukeResult struct {
	DeletedCount int
	Scope        string
	ProjectName  string
}

// Nuke deletes all scratches in the specified scope
func Nuke(s *store.Store, all bool, project string) (*NukeResult, error) {
	var scratchesToDelete []store.Scratch
	result := &NukeResult{}

	if all {
		// Delete all scratches across all scopes
		scratchesToDelete = s.GetScratches()
		result.Scope = "all"
		result.DeletedCount = len(scratchesToDelete)
	} else if project == "" {
		// Delete only global scratches
		for _, scratch := range s.GetScratches() {
			if scratch.Project == "global" {
				scratchesToDelete = append(scratchesToDelete, scratch)
			}
		}
		result.Scope = "global"
		result.DeletedCount = len(scratchesToDelete)
	} else {
		// Delete only project-specific scratches
		for _, scratch := range s.GetScratches() {
			if scratch.Project == project {
				scratchesToDelete = append(scratchesToDelete, scratch)
			}
		}
		result.Scope = "project"
		result.ProjectName = project
		result.DeletedCount = len(scratchesToDelete)
	}

	// Delete the scratch files
	for _, scratch := range scratchesToDelete {
		if err := deleteScratchFile(scratch.ID); err != nil {
			// Continue deleting even if one fails
			fmt.Fprintf(os.Stderr, "Warning: failed to delete file for scratch %s: %v\n", scratch.ID, err)
		}
	}

	// Remove all scratches from the store
	if all {
		// Clear all scratches
		if err := s.SaveScratches([]store.Scratch{}); err != nil {
			return nil, err
		}
	} else {
		// Keep only scratches not in the delete list
		var remainingScratches []store.Scratch
		for _, scratch := range s.GetScratches() {
			shouldDelete := false
			for _, toDelete := range scratchesToDelete {
				if scratch.ID == toDelete.ID {
					shouldDelete = true
					break
				}
			}
			if !shouldDelete {
				remainingScratches = append(remainingScratches, scratch)
			}
		}
		if err := s.SaveScratches(remainingScratches); err != nil {
			return nil, err
		}
	}

	return result, nil
}
