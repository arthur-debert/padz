package commands

import (
	"strings"

	"github.com/arthur-debert/padz/pkg/editor"
	"github.com/arthur-debert/padz/pkg/store"
)

func Open(s *store.Store, global bool, project string, indexStr string) error {
	scratchToOpen, err := GetScratchByIndex(s, global, project, indexStr)
	if err != nil {
		return err
	}

	content, err := readScratchFile(scratchToOpen.ID)
	if err != nil {
		return err
	}

	// Determine extension based on content
	extension := ".txt"
	contentStr := strings.TrimSpace(string(content))
	if strings.HasPrefix(contentStr, "#") {
		extension = ".md"
	}

	newContent, err := editor.OpenInEditorWithExtension(content, extension)
	if err != nil {
		return err
	}

	trimmedContent := trim(newContent)
	if len(trimmedContent) == 0 {
		// If the file is empty, soft delete the scratch
		return Delete(s, global, project, indexStr)
	}

	if err := saveScratchFile(scratchToOpen.ID, trimmedContent); err != nil {
		return err
	}

	scratchToOpen.Title = getTitle(trimmedContent)
	return s.UpdateScratchAtomic(*scratchToOpen)
}

// OpenLazy opens a scratch in the editor and exits immediately (non-blocking)
func OpenLazy(s *store.Store, global bool, project string, indexStr string) error {
	scratchToOpen, err := GetScratchByIndex(s, global, project, indexStr)
	if err != nil {
		return err
	}

	content, err := readScratchFile(scratchToOpen.ID)
	if err != nil {
		return err
	}

	// Launch editor and exit immediately
	return editor.LaunchAndExit(scratchToOpen.ID, content)
}
