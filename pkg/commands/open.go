package commands

import (
	"strings"

	"github.com/arthur-debert/padz/pkg/editor"
	"github.com/arthur-debert/padz/pkg/store"
)

func Open(s *store.Store, all bool, global bool, project string, indexStr string) error {
	scratchToOpen, err := GetScratchByIndex(s, all, global, project, indexStr)
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
		return Delete(s, all, global, project, indexStr)
	}

	if err := saveScratchFile(scratchToOpen.ID, trimmedContent); err != nil {
		return err
	}

	scratchToOpen.Title = getTitle(trimmedContent)
	return s.UpdateScratchAtomic(*scratchToOpen)
}

// OpenLazy opens a scratch in the editor and exits immediately (non-blocking)
func OpenLazy(s *store.Store, all bool, global bool, project string, indexStr string) error {
	scratchToOpen, err := GetScratchByIndex(s, all, global, project, indexStr)
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

// OpenWithStoreManager opens a scratch in the editor using StoreManager
func OpenWithStoreManager(workingDir string, globalFlag bool, indexStr string) error {
	sm := store.NewStoreManager()

	// Resolve the scratch
	scopedScratch, err := ResolveScratchWithStoreManager(workingDir, globalFlag, indexStr)
	if err != nil {
		return err
	}

	content, err := readScratchFile(scopedScratch.ID)
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
		return DeleteWithStoreManager(workingDir, globalFlag, indexStr)
	}

	if err := saveScratchFile(scopedScratch.ID, trimmedContent); err != nil {
		return err
	}

	// Update the scratch in the appropriate store
	var targetStore *store.Store
	if scopedScratch.Scope == "global" {
		targetStore, err = sm.GetGlobalStore()
	} else {
		targetStore, err = sm.GetProjectStore(scopedScratch.Scope, workingDir)
	}
	if err != nil {
		return err
	}

	scopedScratch.Title = getTitle(trimmedContent)
	return targetStore.UpdateScratchAtomic(*scopedScratch.Scratch)
}

// OpenLazyWithStoreManager opens a scratch in the editor and exits immediately using StoreManager
func OpenLazyWithStoreManager(workingDir string, globalFlag bool, indexStr string) error {
	// Resolve the scratch
	scopedScratch, err := ResolveScratchWithStoreManager(workingDir, globalFlag, indexStr)
	if err != nil {
		return err
	}

	content, err := readScratchFile(scopedScratch.ID)
	if err != nil {
		return err
	}

	// Launch editor and exit immediately
	return editor.LaunchAndExit(scopedScratch.ID, content)
}
