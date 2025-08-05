package commands

import (
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/arthur-debert/padz/pkg/testutil"
	"testing"
)

// TestSetup contains everything needed for testing commands
type TestSetup struct {
	Store   *store.Store
	Config  *config.Config
	Cleanup func()
}

// SetupCommandTest prepares a test environment for command tests
func SetupCommandTest(t *testing.T) *TestSetup {
	cfg, cleanup := testutil.SetupTestEnvironment(t)

	s, err := store.NewStoreWithConfig(cfg)
	if err != nil {
		t.Fatalf("Failed to create test store: %v", err)
	}

	return &TestSetup{
		Store:   s,
		Config:  cfg,
		Cleanup: cleanup,
	}
}

// WriteScratchFile writes a scratch file content in the test filesystem
func (ts *TestSetup) WriteScratchFile(t *testing.T, id string, content []byte) {
	path, err := store.GetScratchFilePathWithConfig(id, ts.Config)
	if err != nil {
		t.Fatalf("Failed to get scratch file path: %v", err)
	}

	err = ts.Config.FileSystem.WriteFile(path, content, 0644)
	if err != nil {
		t.Fatalf("Failed to write scratch file: %v", err)
	}
}

// ReadScratchFile reads a scratch file from the test filesystem
func (ts *TestSetup) ReadScratchFile(t *testing.T, id string) []byte {
	path, err := store.GetScratchFilePathWithConfig(id, ts.Config)
	if err != nil {
		t.Fatalf("Failed to get scratch file path: %v", err)
	}

	content, err := ts.Config.FileSystem.ReadFile(path)
	if err != nil {
		t.Fatalf("Failed to read scratch file: %v", err)
	}

	return content
}
