package commands

import (
	"github.com/arthur-debert/padz/pkg/clipboard"
	"github.com/arthur-debert/padz/pkg/store"
)

// Copy retrieves a scratch by index and copies its content to the clipboard
func Copy(s *store.Store, all bool, global bool, project string, indexStr string) error {
	scratch, err := GetScratchByIndex(s, all, global, project, indexStr)
	if err != nil {
		return err
	}

	content, err := readScratchFile(scratch.ID)
	if err != nil {
		return err
	}

	if err := clipboard.Copy(content); err != nil {
		return err
	}

	return nil
}
