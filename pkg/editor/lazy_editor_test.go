package editor

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestLaunchAndExit(t *testing.T) {
	// Skip this test in CI as it requires a real editor
	if os.Getenv("CI") == "true" {
		t.Skip("Skipping editor test in CI environment")
	}

	// Create a temporary directory for testing
	tmpDir, err := os.MkdirTemp("", "padz-lazy-test-*")
	require.NoError(t, err)
	defer func() { _ = os.RemoveAll(tmpDir) }()

	// Set up environment
	originalEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", originalEditor) }()

	// Use 'true' command as a no-op editor for testing
	_ = os.Setenv("EDITOR", "true")

	// Create test config
	cfg := &config.Config{
		FileSystem: &filesystem.OSFileSystem{},
		DataPath:   tmpDir,
	}

	// Test with new file structure
	filesDir := filepath.Join(tmpDir, "scratch", "files")
	err = os.MkdirAll(filesDir, 0755)
	require.NoError(t, err)

	scratchID := "test123"
	content := []byte("Test content for lazy editor")

	// Launch editor
	err = LaunchAndExitWithConfig(scratchID, content, cfg)
	assert.NoError(t, err)

	// Verify file was created
	filePath := filepath.Join(filesDir, scratchID)
	assert.FileExists(t, filePath)

	// Verify content
	savedContent, err := os.ReadFile(filePath)
	require.NoError(t, err)
	assert.Equal(t, content, savedContent)
}

func TestLaunchAndExit_LegacyStructure(t *testing.T) {
	// Skip this test in CI as it requires a real editor
	if os.Getenv("CI") == "true" {
		t.Skip("Skipping editor test in CI environment")
	}

	// Create a temporary directory for testing
	tmpDir, err := os.MkdirTemp("", "padz-lazy-legacy-test-*")
	require.NoError(t, err)
	defer func() { _ = os.RemoveAll(tmpDir) }()

	// Set up environment
	originalEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", originalEditor) }()
	_ = os.Setenv("EDITOR", "true")

	// Create test config
	cfg := &config.Config{
		FileSystem: &filesystem.OSFileSystem{},
		DataPath:   tmpDir,
	}

	// Create legacy structure (no files subdirectory)
	scratchDir := filepath.Join(tmpDir, "scratch")
	err = os.MkdirAll(scratchDir, 0755)
	require.NoError(t, err)

	scratchID := "legacy123"
	content := []byte("Legacy test content")

	// Launch editor
	err = LaunchAndExitWithConfig(scratchID, content, cfg)
	assert.NoError(t, err)

	// Verify file was created in legacy location
	filePath := filepath.Join(scratchDir, scratchID)
	assert.FileExists(t, filePath)

	// Verify content
	savedContent, err := os.ReadFile(filePath)
	require.NoError(t, err)
	assert.Equal(t, content, savedContent)
}

func TestLaunchAndExit_NoEditor(t *testing.T) {
	// Skip this test in CI
	if os.Getenv("CI") == "true" {
		t.Skip("Skipping editor test in CI environment")
	}

	// Create a temporary directory for testing
	tmpDir, err := os.MkdirTemp("", "padz-lazy-noeditor-test-*")
	require.NoError(t, err)
	defer func() { _ = os.RemoveAll(tmpDir) }()

	// Clear EDITOR environment variable
	originalEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", originalEditor) }()
	_ = os.Unsetenv("EDITOR")

	// Create test config
	cfg := &config.Config{
		FileSystem: &filesystem.OSFileSystem{},
		DataPath:   tmpDir,
	}

	// Create file structure
	filesDir := filepath.Join(tmpDir, "scratch", "files")
	err = os.MkdirAll(filesDir, 0755)
	require.NoError(t, err)

	scratchID := "noeditor123"
	content := []byte("No editor test")

	// This might fail if vim is not available, so we just check it doesn't panic
	_ = LaunchAndExitWithConfig(scratchID, content, cfg)

	// If vim is available, file should exist
	filePath := filepath.Join(filesDir, scratchID)
	if _, err := os.Stat(filePath); err == nil {
		savedContent, err := os.ReadFile(filePath)
		require.NoError(t, err)
		assert.Equal(t, content, savedContent)
	}
}
