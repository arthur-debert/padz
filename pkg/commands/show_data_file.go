package commands

import (
	"github.com/arthur-debert/padz/pkg/store"
)

// ShowDataFileResult contains the data file path information
type ShowDataFileResult struct {
	Path string `json:"path"`
}

// ShowDataFile returns the path to the data file used by padz
func ShowDataFile(s *store.Store) (*ShowDataFileResult, error) {
	// Get the base scratch directory path
	path, err := store.GetScratchPath()
	if err != nil {
		return nil, err
	}

	return &ShowDataFileResult{
		Path: path,
	}, nil
}

// ShowDataFileWithStoreManager returns the path to the data file for the current store using StoreManager
func ShowDataFileWithStoreManager(workingDir string, globalFlag bool) (*ShowDataFileResult, error) {
	sm := store.NewStoreManager()

	// Get the current store based on the global flag
	currentStore, _, err := sm.GetCurrentStore(workingDir, globalFlag)
	if err != nil {
		return nil, err
	}

	// Use the existing ShowDataFile function with the current store
	// This will return the path to the scratch directory for this store
	result, err := ShowDataFile(currentStore)
	if err != nil {
		return nil, err
	}

	// Add scope information to make it clear which store we're looking at
	// The path already contains the correct location (global or project-specific)
	return &ShowDataFileResult{
		Path: result.Path,
	}, nil
}
