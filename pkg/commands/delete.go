package commands

import (
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/store"
)

func Delete(s *store.Store, all bool, global bool, project string, indexStr string) error {
	scratchToDelete, err := GetScratchByIndex(s, all, global, project, indexStr)
	if err != nil {
		return err
	}

	// Soft delete: mark as deleted instead of removing
	now := time.Now()
	scratchToDelete.IsDeleted = true
	scratchToDelete.DeletedAt = &now

	return s.UpdateScratchAtomic(*scratchToDelete)
}

// PermanentlyDeleteScratchFile removes the physical file from disk
// This is used by the flush command for hard deletion
func PermanentlyDeleteScratchFile(id string) error {
	fs := config.GetConfig().FileSystem
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		return err
	}
	return fs.Remove(path)
}
