package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
	"time"
)

func Cleanup(s *store.Store, days int) error {
	scratches := s.GetScratches()
	cutoff := time.Now().AddDate(0, 0, -days)

	var scratchesToKeep []store.Scratch
	var scratchesToDelete []store.Scratch

	for _, scratch := range scratches {
		if scratch.CreatedAt.Before(cutoff) {
			scratchesToDelete = append(scratchesToDelete, scratch)
		} else {
			scratchesToKeep = append(scratchesToKeep, scratch)
		}
	}

	for _, scratch := range scratchesToDelete {
		if err := deleteScratchFile(scratch.ID); err != nil {
			return err
		}
	}

	return s.SaveScratchesAtomic(scratchesToKeep)
}
