package commands

import (
	"fmt"
	"strconv"

	"github.com/arthur-debert/padz/pkg/store"
)

// PathResult contains the path information for a scratch
type PathResult struct {
	Path string `json:"path"`
}

// Path returns the full path to a scratch file
func Path(s *store.Store, project string, indexStr string) (*PathResult, error) {
	index, err := strconv.Atoi(indexStr)
	if err != nil {
		return nil, fmt.Errorf("invalid index: %s", indexStr)
	}

	// Get scratches and filter by project
	allScratches := s.GetScratches()
	var scratches []store.Scratch
	
	for _, scratch := range allScratches {
		if scratch.Project == project {
			scratches = append(scratches, scratch)
		}
	}
	
	if len(scratches) == 0 {
		return nil, fmt.Errorf("no scratches found in project %s", project)
	}

	if index < 1 || index > len(scratches) {
		return nil, fmt.Errorf("index %d out of range (1-%d)", index, len(scratches))
	}

	scratch := scratches[index-1]
	path, err := store.GetScratchFilePath(scratch.ID)
	if err != nil {
		return nil, fmt.Errorf("failed to get scratch file path: %w", err)
	}

	return &PathResult{Path: path}, nil
}