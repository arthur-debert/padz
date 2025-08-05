package testutil

import (
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"testing"
)

// SetupTestEnvironment sets up an isolated test environment
// Returns a cleanup function that should be called with defer
func SetupTestEnvironment(t *testing.T) (*config.Config, func()) {
	// Create a memory filesystem for testing
	memFS := filesystem.NewMemoryFileSystem()

	// Create test configuration
	testConfig := &config.Config{
		FileSystem: memFS,
		DataPath:   "/test/data",
	}

	// Save current config
	oldConfig := config.GetConfig()

	// Set test config
	config.SetConfig(testConfig)

	// Create the data directories
	if err := memFS.MkdirAll("/test/data", 0755); err != nil {
		t.Fatalf("Failed to create test data directory: %v", err)
	}
	if err := memFS.MkdirAll("/test/data/scratch", 0755); err != nil {
		t.Fatalf("Failed to create test scratch directory: %v", err)
	}

	// Return cleanup function
	cleanup := func() {
		// Restore original config
		config.SetConfig(oldConfig)
		// Reset the memory filesystem
		memFS.Reset()
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
