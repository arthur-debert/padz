package commands

import (
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/store"
)

func View(s *store.Store, all, global bool, project string, indexStr string) (string, error) {
	scratch, err := GetScratchByIndex(s, all, global, project, indexStr)
	if err != nil {
		return "", err
	}

	content, err := readScratchFile(scratch.ID)
	if err != nil {
		return "", err
	}

	return string(content), nil
}

func readScratchFile(id string) ([]byte, error) {
	fs := config.GetConfig().FileSystem
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		return nil, err
	}
	return fs.ReadFile(path)
}
