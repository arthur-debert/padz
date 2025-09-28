package store

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestMigrateProjectPaths(t *testing.T) {

	t.Run("migrates absolute paths to relative project names", func(t *testing.T) {
		// Create a temporary directory structure
		tempDir := t.TempDir()
		projectDir := filepath.Join(tempDir, "myproject")
		err := os.MkdirAll(filepath.Join(projectDir, ".git"), 0755)
		require.NoError(t, err)

		// Change to project directory
		oldWd, _ := os.Getwd()
		err = os.Chdir(projectDir)
		require.NoError(t, err)
		defer func() {
			_ = os.Chdir(oldWd)
		}()

		// Create store with test data containing absolute paths
		fs := filesystem.NewMemoryFileSystem()
		cfg := &config.Config{
			FileSystem:    fs,
			DataPath:      tempDir,
			IsGlobalScope: false,
		}

		store := &Store{
			fs:  fs,
			cfg: cfg,
			scratches: []Scratch{
				{
					ID:        "test1",
					Project:   projectDir, // Absolute path
					Title:     "Test 1",
					CreatedAt: time.Now(),
				},
				{
					ID:        "test2",
					Project:   filepath.Join("/some/other/path", "myproject"), // Different absolute path with same project name
					Title:     "Test 2",
					CreatedAt: time.Now(),
				},
				{
					ID:        "test3",
					Project:   "myproject", // Already relative
					Title:     "Test 3",
					CreatedAt: time.Now(),
				},
				{
					ID:        "test4",
					Project:   "/completely/different/project", // Different project - should not migrate
					Title:     "Test 4",
					CreatedAt: time.Now(),
				},
			},
		}

		// Run migration
		err = store.migrateProjectPaths()
		require.NoError(t, err)

		// Check results
		assert.Equal(t, "myproject", store.scratches[0].Project, "Should migrate absolute project path to relative")
		assert.Equal(t, "myproject", store.scratches[1].Project, "Should migrate matching project name from different path")
		assert.Equal(t, "myproject", store.scratches[2].Project, "Should keep already relative project name")
		assert.Equal(t, "/completely/different/project", store.scratches[3].Project, "Should not migrate different project")
	})

	t.Run("does nothing in global scope", func(t *testing.T) {
		fs := filesystem.NewMemoryFileSystem()
		cfg := &config.Config{
			FileSystem:    fs,
			DataPath:      t.TempDir(),
			IsGlobalScope: true,
		}

		store := &Store{
			fs:  fs,
			cfg: cfg,
			scratches: []Scratch{
				{
					ID:        "test1",
					Project:   "/some/absolute/path",
					Title:     "Test 1",
					CreatedAt: time.Now(),
				},
			},
		}

		// Run migration
		err := store.migrateProjectPaths()
		require.NoError(t, err)

		// Should not change anything in global scope
		assert.Equal(t, "/some/absolute/path", store.scratches[0].Project)
	})

	t.Run("handles empty scratches", func(t *testing.T) {
		fs := filesystem.NewMemoryFileSystem()
		cfg := &config.Config{
			FileSystem:    fs,
			DataPath:      t.TempDir(),
			IsGlobalScope: false,
		}

		store := &Store{
			fs:        fs,
			cfg:       cfg,
			scratches: []Scratch{},
		}

		// Should not error with empty scratches
		err := store.migrateProjectPaths()
		require.NoError(t, err)
	})
}

func TestGetProjectRoot(t *testing.T) {
	t.Run("finds project root with .git directory", func(t *testing.T) {
		// Create a temporary directory structure
		tempDir := t.TempDir()
		projectDir := filepath.Join(tempDir, "myproject")
		subDir := filepath.Join(projectDir, "src", "components")
		gitDir := filepath.Join(projectDir, ".git")

		err := os.MkdirAll(gitDir, 0755)
		require.NoError(t, err)
		err = os.MkdirAll(subDir, 0755)
		require.NoError(t, err)

		// Test from subdirectory
		root, err := project.GetProjectRoot(subDir)
		require.NoError(t, err)
		assert.Equal(t, projectDir, root)

		// Test from project root
		root, err = project.GetProjectRoot(projectDir)
		require.NoError(t, err)
		assert.Equal(t, projectDir, root)
	})

	t.Run("returns empty string when not in project", func(t *testing.T) {
		tempDir := t.TempDir()

		root, err := project.GetProjectRoot(tempDir)
		require.NoError(t, err)
		assert.Empty(t, root)
	})
}