package project

import (
	"path/filepath"

	"github.com/arthur-debert/padz/pkg/config"
)

func GetCurrentProject(dir string) (string, error) {
	cfg := config.GetConfig()
	fs := cfg.FileSystem

	for {
		gitPath := fs.Join(dir, ".git")
		if _, err := fs.Stat(gitPath); err == nil {
			return filepath.Base(dir), nil
		}

		parent := filepath.Dir(dir)
		if parent == dir {
			return "global", nil
		}
		dir = parent
	}
}
