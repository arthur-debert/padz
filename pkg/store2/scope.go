package store2

import (
	"os"
	"path/filepath"

	"github.com/adrg/xdg"
)

// DetectScope detects whether we're in a project or global context
func DetectScope(dir string) (scope string, err error) {
	// Walk up directory tree looking for .git
	for {
		gitPath := filepath.Join(dir, ".git")
		if _, err := os.Stat(gitPath); err == nil {
			// Found git repo, use parent dir name as scope
			return filepath.Base(dir), nil
		}

		parent := filepath.Dir(dir)
		if parent == dir {
			// Reached root, use global scope
			return "global", nil
		}
		dir = parent
	}
}

// GetStorePath returns the path for a store based on scope
func GetStorePath(scope string) (string, error) {
	// Use XDG data directory for macOS compatibility
	baseDir, err := xdg.DataFile("padz/store2")
	if err != nil {
		return "", err
	}

	// For empty scope, return base directory
	if scope == "" {
		return baseDir, nil
	}

	// Create scope-specific subdirectory
	storePath := filepath.Join(baseDir, scope)
	return storePath, nil
}
