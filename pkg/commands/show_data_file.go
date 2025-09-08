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
