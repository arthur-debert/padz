package commands

import (
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/store"
)

func Delete(s *store.Store, all bool, project string, indexStr string) error {
	scratchToDelete, err := GetScratchByIndex(s, all, false, project, indexStr)
	if err != nil {
		return err
	}

	if err := deleteScratchFile(scratchToDelete.ID); err != nil {
		return err
	}

	return s.RemoveScratch(scratchToDelete.ID)
}

func deleteScratchFile(id string) error {
	fs := config.GetConfig().FileSystem
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		return err
	}
	return fs.Remove(path)
}
