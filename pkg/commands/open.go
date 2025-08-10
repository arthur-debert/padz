package commands

import (
	"github.com/arthur-debert/padz/pkg/editor"
	"github.com/arthur-debert/padz/pkg/store"
)

func Open(s *store.Store, all bool, project string, indexStr string) error {
	scratchToOpen, err := GetScratchByIndex(s, all, false, project, indexStr)
	if err != nil {
		return err
	}

	content, err := readScratchFile(scratchToOpen.ID)
	if err != nil {
		return err
	}

	newContent, err := editor.OpenInEditor(content)
	if err != nil {
		return err
	}

	trimmedContent := trim(newContent)
	if len(trimmedContent) == 0 {
		// If the file is empty, delete the scratch
		if err := deleteScratchFile(scratchToOpen.ID); err != nil {
			return err
		}
		return s.RemoveScratchAtomic(scratchToOpen.ID)
	}

	if err := saveScratchFile(scratchToOpen.ID, trimmedContent); err != nil {
		return err
	}

	scratchToOpen.Title = getTitle(trimmedContent)
	return s.UpdateScratchAtomic(*scratchToOpen)
}
