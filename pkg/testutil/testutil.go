package testutil

import (
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"testing"
)

// SetupTestEnvironment sets up an isolated test environment
// Returns a cleanup function that should be called with defer
func SetupTestEnvironment(t *testing.T) (*config.Config, func()) {
	// Create a temporary directory for testing
	tempDir := t.TempDir()

	// Use OS filesystem since nanostore needs real file access
	osFS := filesystem.NewOSFileSystem()

	// Create test configuration
	testConfig := &config.Config{
		FileSystem: osFS,
		DataPath:   tempDir,
	}

	// Save current config
	oldConfig := config.GetConfig()

	// Set test config
	config.SetConfig(testConfig)

	// Return cleanup function
	cleanup := func() {
		// Restore original config
		config.SetConfig(oldConfig)
		// t.TempDir() automatically cleans up
	}

	return testConfig, cleanup
}

// GetMemoryFS extracts the memory filesystem from a config
func GetMemoryFS(cfg *config.Config) *filesystem.MemoryFileSystem {
	if memFS, ok := cfg.FileSystem.(*filesystem.MemoryFileSystem); ok {
		return memFS
	}
	return nil
}
