package testutil

import (
	"github.com/arthur-debert/padz/pkg/config"
	"testing"
)

// SetupTestEnvironment sets up an isolated test environment
// Returns a cleanup function that should be called with defer
func SetupTestEnvironment(t *testing.T) (*config.Config, func()) {
	// Create a temporary directory for testing
	tempDir := t.TempDir()

	// Create test configuration
	testConfig := &config.Config{
		DataPath: tempDir,
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
