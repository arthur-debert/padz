package commands

import (
	"fmt"
	"os"
	"github.com/arthur-debert/padz/pkg/store"
	"strconv"
)

func Delete(s *store.Store, project string, indexStr string) error {
	scratches := Ls(s, false, false, project)

	index, err := strconv.Atoi(indexStr)
	if err != nil {
		return fmt.Errorf("invalid index: %s", indexStr)
	}

	if index < 1 || index > len(scratches) {
		return fmt.Errorf("index out of range: %d", index)
	}

	scratchToDelete := scratches[index-1]

	if err := deleteScratchFile(scratchToDelete.ID); err != nil {
		return err
	}

	return s.RemoveScratch(scratchToDelete.ID)
}

func deleteScratchFile(id string) error {
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		return err
	}
	return os.Remove(path)
}
