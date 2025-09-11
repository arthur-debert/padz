package commands

import (
	"fmt"

	"github.com/arthur-debert/padz/pkg/store"
)

// PathResult contains the path information for a scratch
type PathResult struct {
	Path string `json:"path"`
}

// Path returns the full path to a scratch file
func Path(s *store.Store, all bool, global bool, project string, indexStr string) (*PathResult, error) {
	scratch, err := GetScratchByIndex(s, all, global, project, indexStr)
	if err != nil {
		return nil, err
	}

	path, err := store.GetScratchFilePath(scratch.ID)
	if err != nil {
		return nil, fmt.Errorf("failed to get scratch file path: %w", err)
	}

	return &PathResult{Path: path}, nil
}

// PathWithStoreManager returns the full path to a scratch file using StoreManager
func PathWithStoreManager(workingDir string, globalFlag bool, indexStr string) (*PathResult, error) {
	// Resolve the scratch
	scopedScratch, err := ResolveScratchWithStoreManager(workingDir, globalFlag, indexStr)
	if err != nil {
		return nil, err
	}

	path, err := store.GetScratchFilePath(scopedScratch.ID)
	if err != nil {
		return nil, fmt.Errorf("failed to get scratch file path: %w", err)
	}

	return &PathResult{Path: path}, nil
}
