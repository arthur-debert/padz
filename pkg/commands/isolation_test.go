package commands

import (
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/adrg/xdg"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/arthur-debert/padz/pkg/testutil"
)

// TestIsolation demonstrates that tests are fully isolated from the real filesystem
func TestIsolation(t *testing.T) {
	// Save current home directory
	homeDir, err := os.UserHomeDir()
	if err != nil {
		t.Fatalf("Failed to get home directory: %v", err)
	}

	// Create a path that would be in the real home directory
	realScratchPath := filepath.Join(homeDir, ".local", "share", "scratch")

	// Setup test environment
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create several scratches
	testContent := "Test content that should NEVER appear in real home directory"
	err = Create(setup.Store, "test-project", []byte(testContent))
	if err != nil {
		t.Fatalf("Failed to create scratch: %v", err)
	}

	// Verify scratch was created
	scratches := setup.Store.GetScratches()
	if len(scratches) != 1 {
		t.Fatalf("Expected 1 scratch, got %d", len(scratches))
	}

	// Get the memory filesystem
	memFS := testutil.GetMemoryFS(setup.Config)
	if memFS == nil {
		t.Fatal("Expected memory filesystem")
	}

	// Verify files exist in memory filesystem
	files := memFS.GetAllFiles()
	t.Logf("Files in memory filesystem:")
	for path, content := range files {
		t.Logf("  %s (%d bytes)", path, len(content))
	}

	// CRITICAL: Verify NO test files were created in the real filesystem
	// Note: We check if our specific test ID exists, not just any scratch directory
	testScratchID := scratches[0].ID

	// Check various possible locations where files might be created
	possiblePaths := []string{
		realScratchPath,
		filepath.Join(homeDir, ".scratch"),
		filepath.Join(homeDir, "scratch"),
	}

	for _, checkPath := range possiblePaths {
		if _, err := os.Stat(checkPath); err == nil {
			// Directory exists, check if our test file is in it
			entries, _ := os.ReadDir(checkPath)
			for _, entry := range entries {
				if entry.Name() == testScratchID {
					t.Errorf("Test created file in real directory %s: %s", checkPath, entry.Name())
				}
			}
		}
	}

	// Demonstrate that XDG paths are not being used in tests
	xdgDataHome := xdg.DataHome
	testDataPath := setup.Config.DataPath
	if testDataPath == xdgDataHome || testDataPath == "" {
		t.Errorf("Test is using real XDG data home: %s", xdgDataHome)
	}

	t.Logf("Test successfully isolated:")
	t.Logf("  - Real XDG data home: %s", xdgDataHome)
	t.Logf("  - Test data path: %s", testDataPath)
	t.Logf("  - Files exist only in memory, not on disk")
}

// TestMemoryFilesystemPersistence verifies that each test gets a fresh filesystem
func TestMemoryFilesystemPersistence(t *testing.T) {
	// First test - create a file
	t.Run("CreateFile", func(t *testing.T) {
		setup := SetupCommandTest(t)
		defer setup.Cleanup()

		err := Create(setup.Store, "test", []byte("Test 1 content"))
		if err != nil {
			t.Fatalf("Failed to create scratch: %v", err)
		}

		scratches := setup.Store.GetScratches()
		if len(scratches) != 1 {
			t.Errorf("Expected 1 scratch, got %d", len(scratches))
		}
	})

	// Second test - verify previous test's data is gone
	t.Run("VerifyCleanSlate", func(t *testing.T) {
		setup := SetupCommandTest(t)
		defer setup.Cleanup()

		// Should start with no scratches
		scratches := setup.Store.GetScratches()
		if len(scratches) != 0 {
			t.Errorf("Expected 0 scratches (clean slate), got %d", len(scratches))
		}

		// Create a different scratch
		err := Create(setup.Store, "test", []byte("Test 2 content"))
		if err != nil {
			t.Fatalf("Failed to create scratch: %v", err)
		}

		// Verify we have exactly one (our new one)
		scratches = setup.Store.GetScratches()
		if len(scratches) != 1 {
			t.Errorf("Expected 1 scratch, got %d", len(scratches))
		}

		if scratches[0].Title != "Test 2 content" {
			t.Errorf("Found unexpected content from previous test")
		}
	})
}

// TestNoRealFileSystemAccess verifies the abstraction prevents real filesystem access
func TestNoRealFileSystemAccess(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	// Try to use store functions that would normally touch the filesystem
	s, err := store.NewStoreWithConfig(cfg)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}

	// Create a scratch
	scratch := store.Scratch{
		ID:      "test-no-fs-access",
		Project: "test",
		Title:   "Test",
	}

	err = s.AddScratch(scratch)
	if err != nil {
		t.Fatalf("Failed to add scratch: %v", err)
	}

	// Get the path where the file would be
	path, err := store.GetScratchFilePathWithConfig(scratch.ID, cfg)
	if err != nil {
		t.Fatalf("Failed to get path: %v", err)
	}

	// Verify the path is in our test location, not real filesystem
	if !strings.HasPrefix(path, "/test/data") {
		t.Errorf("Path is not in test location: %s", path)
	}

	// Write content using our abstracted filesystem
	content := []byte("This should only exist in memory")
	err = cfg.FileSystem.WriteFile(path, content, 0644)
	if err != nil {
		t.Fatalf("Failed to write file: %v", err)
	}

	// Verify we can read it back from memory
	readContent, err := cfg.FileSystem.ReadFile(path)
	if err != nil {
		t.Fatalf("Failed to read file: %v", err)
	}

	if string(readContent) != string(content) {
		t.Errorf("Content mismatch")
	}

	// CRITICAL: Verify the file does NOT exist on real filesystem
	if _, err := os.Stat(path); err == nil {
		t.Errorf("File exists on real filesystem at %s - isolation failed!", path)
	}
}
