package commands

import (
	"fmt"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// Pin marks a scratch as pinned
func Pin(s *store.Store, all, global bool, project string, id string) error {
	// Resolve the scratch
	scratch, err := ResolveScratchID(s, all, global, project, id)
	if err != nil {
		return err
	}

	// Check if already pinned
	if scratch.IsPinned {
		return fmt.Errorf("scratch is already pinned")
	}

	// Check pinned limit
	pinned := s.GetPinnedScratches()
	if len(pinned) >= store.MaxPinnedScratches {
		return fmt.Errorf("maximum number of pinned scratches (%d) reached", store.MaxPinnedScratches)
	}

	// Update the scratch
	scratch.IsPinned = true
	scratch.PinnedAt = time.Now()

	// Save using atomic update
	return s.UpdateScratchAtomic(*scratch)
}

// Unpin removes the pinned status from a scratch
func Unpin(s *store.Store, all, global bool, project string, id string) error {
	// Resolve the scratch
	scratch, err := ResolveScratchID(s, all, global, project, id)
	if err != nil {
		return err
	}

	// Check if not pinned
	if !scratch.IsPinned {
		return fmt.Errorf("scratch is not pinned")
	}

	// Update the scratch
	scratch.IsPinned = false
	scratch.PinnedAt = time.Time{} // Zero value
	return s.UpdateScratchAtomic(*scratch)
}
