package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
)

func Ls(s *store.Store, all, global bool, project string) []store.Scratch {
	scratches := s.GetScratches()
	if all {
		return scratches
	}

	var filtered []store.Scratch
	for _, scratch := range scratches {
		if global && scratch.Project == "global" {
			filtered = append(filtered, scratch)
		} else if !global && scratch.Project == project {
			filtered = append(filtered, scratch)
		}
	}
	return filtered
}
