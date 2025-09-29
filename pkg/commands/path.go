package commands

import (
	"fmt"

	"github.com/arthur-debert/padz/pkg/store"
)

// PathResult contains the path information for a scratch
type PathResult struct {
	Path string `json:"path"`
}

// Path returns the identifier path for a scratch (now stored in nanostore)
func Path(s *store.Store, global bool, project string, indexStr string) (*PathResult, error) {
	scratch, err := GetScratchByIndex(s, global, project, indexStr)
	if err != nil {
		return nil, err
	}

	// Since scratches are now stored in nanostore, return a virtual path
	// that includes the UUID for identification
	path := fmt.Sprintf("nanostore://scratch/%s", scratch.ID)

	return &PathResult{Path: path}, nil
}
