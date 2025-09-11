package commands

import (
	"fmt"
	"strings"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
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

// NukeWithStoreManager soft-deletes all scratches using StoreManager approach
func NukeWithStoreManager(workingDir string, globalFlag bool, all bool) (*NukeResult, error) {
	sm := store.NewStoreManager()
	now := time.Now()
	result := &NukeResult{}
	totalDeleted := 0

	if all {
		// Nuke all stores - global and current project if available
		result.Scope = "all"

		// Nuke global store
		globalStore, err := sm.GetGlobalStore()
		if err != nil {
			return nil, fmt.Errorf("failed to get global store: %w", err)
		}

		deleted, err := nukeSingleStore(globalStore, now)
		if err != nil {
			return nil, fmt.Errorf("failed to nuke global store: %w", err)
		}
		totalDeleted += deleted

		// Try to nuke current project store if we're in one
		currentStore, _, err := sm.GetCurrentStore(workingDir, false)
		if err == nil && currentStore != globalStore { // Make sure it's not the same as global
			deleted, err = nukeSingleStore(currentStore, now)
			if err != nil {
				// Log but continue - don't fail the whole operation
				log.Error().Err(err).Msg("Failed to nuke current project store")
			} else {
				totalDeleted += deleted
			}
		}
	} else {
		// Nuke specific store based on global flag and current context
		currentStore, scope, err := sm.GetCurrentStore(workingDir, globalFlag)
		if err != nil {
			return nil, fmt.Errorf("failed to get current store: %w", err)
		}

		deleted, err := nukeSingleStore(currentStore, now)
		if err != nil {
			return nil, fmt.Errorf("failed to nuke store %s: %w", scope, err)
		}
		totalDeleted = deleted

		if scope == "global" {
			result.Scope = "global"
		} else {
			result.Scope = "project"
			// Extract project name from scope
			if strings.HasPrefix(scope, "project:") {
				result.ProjectName = strings.TrimPrefix(scope, "project:")
			} else {
				result.ProjectName = scope
			}
		}
	}

	result.DeletedCount = totalDeleted
	return result, nil
}

// nukeSingleStore soft-deletes all scratches in a single store
func nukeSingleStore(s *store.Store, now time.Time) (int, error) {
	scratches := s.GetScratches()
	deletedCount := 0

	// Soft delete all non-deleted scratches
	for i := range scratches {
		if !scratches[i].IsDeleted {
			scratches[i].IsDeleted = true
			scratches[i].DeletedAt = &now
			deletedCount++
		}
	}

	// Save atomically
	if err := s.SaveScratchesAtomic(scratches); err != nil {
		return 0, err
	}

	return deletedCount, nil
}
