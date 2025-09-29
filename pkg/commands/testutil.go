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
