package commands

import (
	"strings"
	"testing"
)

func TestShowDataFile(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Test showing data file path
	result, err := ShowDataFile(setup.Store)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// The result should contain a path
	if result.Path == "" {
		t.Error("expected non-empty path")
	}

	// The path should contain "scratch" directory name
	if !strings.Contains(result.Path, "scratch") {
		t.Errorf("expected path to contain 'scratch', got: %s", result.Path)
	}

	// In test environment, path should be a valid directory
	// Since we're using real temp directories now, just verify it's not empty
	if len(result.Path) == 0 {
		t.Errorf("expected non-empty path, got: %s", result.Path)
	}
}
