package commands

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/arthur-debert/padz/pkg/testutil"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestExport(t *testing.T) {
	t.Run("export all scratches as txt", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create test scratches
		scratches := []struct {
			title   string
			content string
		}{
			{"First Scratch", "This is the first scratch content"},
			{"Second Note", "This is the second note content"},
			{"Third Item", "This is the third item content"},
		}

		for _, s := range scratches {
			err := CreateWithTitle(st, project, []byte(s.content), s.title)
			require.NoError(t, err)
		}

		// Change to temp directory
		tmpDir := t.TempDir()
		oldDir, err := os.Getwd()
		require.NoError(t, err)
		defer func() {
			if err := os.Chdir(oldDir); err != nil {
				t.Logf("Failed to change back to original directory: %v", err)
			}
		}()
		err = os.Chdir(tmpDir)
		require.NoError(t, err)

		// Export all
		err = Export(st, false, false, project, nil, "txt")
		assert.NoError(t, err)

		// Check export directory exists
		entries, err := os.ReadDir(".")
		require.NoError(t, err)
		assert.Equal(t, 1, len(entries))
		assert.True(t, entries[0].IsDir())
		assert.True(t, strings.HasPrefix(entries[0].Name(), "padz-export-"))

		// Check exported files
		exportDir := entries[0].Name()
		files, err := os.ReadDir(exportDir)
		require.NoError(t, err)
		assert.Equal(t, 3, len(files))

		// Check filenames and content (in reverse chronological order)
		expectedFiles := []struct {
			name    string
			content string
		}{
			{"1-third-item.txt", "This is the third item content"},
			{"2-second-note.txt", "This is the second note content"},
			{"3-first-scratch.txt", "This is the first scratch content"},
		}

		for i, expected := range expectedFiles {
			assert.Equal(t, expected.name, files[i].Name())
			content, err := os.ReadFile(filepath.Join(exportDir, files[i].Name()))
			require.NoError(t, err)
			assert.Equal(t, expected.content, string(content))
		}
	})

	t.Run("export specific scratches", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create test scratches
		for i := 1; i <= 5; i++ {
			content := fmt.Sprintf("Content for scratch %d", i)
			err := Create(st, project, []byte(content))
			require.NoError(t, err)
		}

		// Change to temp directory
		tmpDir := t.TempDir()
		oldDir, err := os.Getwd()
		require.NoError(t, err)
		defer func() {
			if err := os.Chdir(oldDir); err != nil {
				t.Logf("Failed to change back to original directory: %v", err)
			}
		}()
		err = os.Chdir(tmpDir)
		require.NoError(t, err)

		// Export specific ones
		err = Export(st, false, false, project, []string{"1", "3", "5"}, "txt")
		assert.NoError(t, err)

		// Check exported files
		entries, err := os.ReadDir(".")
		require.NoError(t, err)
		exportDir := entries[0].Name()
		files, err := os.ReadDir(exportDir)
		require.NoError(t, err)
		assert.Equal(t, 3, len(files))

		// Files should be numbered 1, 2, 3 based on export order
		assert.Equal(t, "1-content-for-scratch-5.txt", files[0].Name())
		assert.Equal(t, "2-content-for-scratch-3.txt", files[1].Name())
		assert.Equal(t, "3-content-for-scratch-1.txt", files[2].Name())
	})

	t.Run("export as markdown", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create a scratch
		err = CreateWithTitle(st, project, []byte("# Markdown Content\n\nThis is markdown"), "Markdown Content")
		require.NoError(t, err)

		// Change to temp directory
		tmpDir := t.TempDir()
		oldDir, err := os.Getwd()
		require.NoError(t, err)
		defer func() {
			if err := os.Chdir(oldDir); err != nil {
				t.Logf("Failed to change back to original directory: %v", err)
			}
		}()
		err = os.Chdir(tmpDir)
		require.NoError(t, err)

		// Export as markdown
		err = Export(st, false, false, project, nil, "markdown")
		assert.NoError(t, err)

		// Check file extension
		entries, err := os.ReadDir(".")
		require.NoError(t, err)
		exportDir := entries[0].Name()
		files, err := os.ReadDir(exportDir)
		require.NoError(t, err)
		assert.Equal(t, 1, len(files))
		assert.True(t, strings.HasSuffix(files[0].Name(), ".md"))
		assert.Equal(t, "1-markdown-content.md", files[0].Name())
	})

	t.Run("export pinned scratches", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create and pin some scratches
		for i := 1; i <= 3; i++ {
			err := Create(st, project, []byte(fmt.Sprintf("Content %d", i)))
			require.NoError(t, err)
		}

		// Pin the second one
		err = Pin(st, false, false, project, "2")
		require.NoError(t, err)

		// Change to temp directory
		tmpDir := t.TempDir()
		oldDir, err := os.Getwd()
		require.NoError(t, err)
		defer func() {
			if err := os.Chdir(oldDir); err != nil {
				t.Logf("Failed to change back to original directory: %v", err)
			}
		}()
		err = os.Chdir(tmpDir)
		require.NoError(t, err)

		// Export using pinned index
		err = Export(st, false, false, project, []string{"p1"}, "txt")
		assert.NoError(t, err)

		// Check exported file
		entries, err := os.ReadDir(".")
		require.NoError(t, err)
		exportDir := entries[0].Name()
		files, err := os.ReadDir(exportDir)
		require.NoError(t, err)
		assert.Equal(t, 1, len(files))
		assert.Equal(t, "1-content-2.txt", files[0].Name())
	})

	t.Run("error on non-existent scratch", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Try to export non-existent scratch
		err = Export(st, false, false, project, []string{"999"}, "txt")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "scratch not found: 999")
	})

	t.Run("error on no scratches to export", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Try to export from empty store
		err = Export(st, false, false, project, nil, "txt")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "no scratches to export")
	})

	t.Run("filename sanitization", func(t *testing.T) {
		// Test cases for filename generation
		testCases := []struct {
			title    string
			expected string
		}{
			{"Title With Spaces", "1-title-with-spaces.txt"},
			{"UPPERCASE TITLE", "1-uppercase-title.txt"},
			{"Title@With#Special$Chars%", "1-titlewithspecialchars.txt"},
			{"Title-With-Dashes", "1-title-with-dashes.txt"},
			{"Very Long Title That Should Be Truncated To Twenty Four", "1-very-long-title-that-sho.txt"},
			{"   Leading and Trailing Spaces   ", "1-leading-and-trailing-spa.txt"},
		}

		for i, tc := range testCases {
			t.Run(tc.title, func(t *testing.T) {
				cfg, cleanup := testutil.SetupTestEnvironment(t)
				defer cleanup()

				st, err := store.NewStoreWithConfig(cfg)
				require.NoError(t, err)
				project := "test-project"

				// Change to temp directory
				tmpDir := t.TempDir()
				oldDir, err := os.Getwd()
				require.NoError(t, err)
				defer func() {
					if err := os.Chdir(oldDir); err != nil {
						t.Logf("Failed to change back to original directory: %v", err)
					}
				}()
				err = os.Chdir(tmpDir)
				require.NoError(t, err)

				// Create scratch with specific title
				err = CreateWithTitle(st, project, []byte("content"), tc.title)
				require.NoError(t, err)

				// Export the scratch
				err = Export(st, false, false, project, nil, "txt")
				assert.NoError(t, err)

				// Check filename
				entries, err := os.ReadDir(".")
				require.NoError(t, err)
				assert.Equal(t, 1, len(entries), "Test case %d: %s", i, tc.title)
				exportDir := entries[0].Name()
				files, err := os.ReadDir(exportDir)
				require.NoError(t, err)
				assert.Equal(t, 1, len(files), "Test case %d: %s", i, tc.title)
				assert.Equal(t, tc.expected, files[0].Name(), "Test case %d: %s", i, tc.title)
			})
		}
	})
}

func TestGenerateFilename(t *testing.T) {
	testCases := []struct {
		name     string
		index    int
		title    string
		format   string
		expected string
	}{
		{"simple txt", 1, "Hello World", "txt", "1-hello-world.txt"},
		{"simple md", 1, "Hello World", "markdown", "1-hello-world.md"},
		{"with special chars", 2, "Test@#$%File", "txt", "2-testfile.txt"},
		{"uppercase", 3, "UPPERCASE", "txt", "3-uppercase.txt"},
		{"long title", 4, "This is a very long title that should be truncated", "txt", "4-this-is-a-very-long-titl.txt"},
		{"trailing spaces", 5, "  spaces  ", "txt", "5-spaces.txt"},
		{"multiple dashes", 6, "one--two---three", "txt", "6-one--two---three.txt"},
		{"ending with dash", 7, "ending-", "txt", "7-ending.txt"},
		{"only special chars", 8, "@#$%^&*()", "txt", "8-.txt"},
	}

	for _, tc := range testCases {
		t.Run(tc.name, func(t *testing.T) {
			result := generateFilename(tc.index, tc.title, tc.format)
			assert.Equal(t, tc.expected, result)
		})
	}
}
