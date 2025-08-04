package commands

import (
	"fmt"
	"os"
	"github.com/arthur-debert/padz/pkg/store"
	"strconv"
)

func View(s *store.Store, all, global bool, project string, indexStr string) (string, error) {
	scratches := Ls(s, all, global, project)

	index, err := strconv.Atoi(indexStr)
	if err != nil {
		return "", fmt.Errorf("invalid index: %s", indexStr)
	}

	if index < 1 || index > len(scratches) {
		return "", fmt.Errorf("index out of range: %d", index)
	}

	scratch := scratches[index-1]

	content, err := readScratchFile(scratch.ID)
	if err != nil {
		return "", err
	}

	return string(content), nil
}

func readScratchFile(id string) ([]byte, error) {
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		return nil, err
	}
	return os.ReadFile(path)
}
