package project

import (
	"os"
	"path/filepath"
)

func GetCurrentProject(dir string) (string, error) {
	projectRoot, err := GetProjectRoot(dir)
	if err != nil {
		return "", err
	}
	if projectRoot == "" {
		return "global", nil
	}
	return filepath.Base(projectRoot), nil
}

// GetProjectRoot returns the root directory path of the current project, or empty string if not in a project
func GetProjectRoot(dir string) (string, error) {
	for {
		if _, err := os.Stat(filepath.Join(dir, ".git")); err == nil {
			return dir, nil
		}

		parent := filepath.Dir(dir)
		if parent == dir {
			return "", nil // Not in a project
		}
		dir = parent
	}
}
