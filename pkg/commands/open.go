package commands

import (
	"fmt"
	"github.com/arthur-debert/padz/pkg/editor"
	"github.com/arthur-debert/padz/pkg/store"
	"strconv"
)

func Open(s *store.Store, project string, indexStr string) error {
	scratches := Ls(s, false, false, project)

	index, err := strconv.Atoi(indexStr)
	if err != nil {
		return fmt.Errorf("invalid index: %s", indexStr)
	}

	if index < 1 || index > len(scratches) {
		return fmt.Errorf("index out of range: %d", index)
	}

	scratchToOpen := scratches[index-1]

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
		return s.RemoveScratch(scratchToOpen.ID)
	}

	if err := saveScratchFile(scratchToOpen.ID, trimmedContent); err != nil {
		return err
	}

	scratchToOpen.Title = getTitle(trimmedContent)
	return s.UpdateScratch(scratchToOpen)
}
