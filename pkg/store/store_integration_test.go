//go:build integration
// +build integration

package store

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestStoreAtomicOperationsIntegration(t *testing.T) {
	if testing.Short() {
		t.Skip("Skipping integration test in short mode")
	}

	t.Run("concurrent AddScratchAtomic prevents duplicates", func(t *testing.T) {
		tempDir, err := os.MkdirTemp("", "padz_test_*")
		require.NoError(t, err)
		defer os.RemoveAll(tempDir)

		cfg := &config.Config{
			DataPath:   tempDir,
			FileSystem: filesystem.NewOSFileSystem(),
		}

		store, err := NewStoreWithConfig(cfg)
		require.NoError(t, err)

		// Create a scratch to add concurrently
		scratch := Scratch{
			ID:        "test123",
			Project:   "test",
			Title:     "Test Scratch",
			CreatedAt: time.Now(),
		}

		// Try to add the same scratch from multiple goroutines
		const numGoroutines = 10
		var wg sync.WaitGroup

		for i := 0; i < numGoroutines; i++ {
			wg.Add(1)
			go func() {
				defer wg.Done()
				_ = store.AddScratchAtomic(scratch)
			}()
		}

		wg.Wait()

		// Even with concurrent adds, we should have only one scratch
		scratches := store.GetScratches()
		assert.Equal(t, 1, len(scratches))
		assert.Equal(t, scratch.ID, scratches[0].ID)
	})

	t.Run("concurrent metadata updates don't lose data", func(t *testing.T) {
		tempDir, err := os.MkdirTemp("", "padz_test_*")
		require.NoError(t, err)
		defer os.RemoveAll(tempDir)

		cfg := &config.Config{
			DataPath:   tempDir,
			FileSystem: filesystem.NewOSFileSystem(),
		}

		store, err := NewStoreWithConfig(cfg)
		require.NoError(t, err)

		// Add scratches concurrently
		const numScratches = 20
		var wg sync.WaitGroup

		for i := 0; i < numScratches; i++ {
			wg.Add(1)
			go func(idx int) {
				defer wg.Done()
				scratch := Scratch{
					ID:        fmt.Sprintf("scratch%d", idx),
					Project:   "test",
					Title:     fmt.Sprintf("Scratch %d", idx),
					CreatedAt: time.Now(),
				}
				err := store.AddScratchAtomic(scratch)
				assert.NoError(t, err)
			}(i)
		}

		wg.Wait()

		// All scratches should be present
		scratches := store.GetScratches()
		assert.Equal(t, numScratches, len(scratches))

		// Verify each scratch exists
		idMap := make(map[string]bool)
		for _, s := range scratches {
			idMap[s.ID] = true
		}
		for i := 0; i < numScratches; i++ {
			expectedID := fmt.Sprintf("scratch%d", i)
			assert.True(t, idMap[expectedID], "Missing scratch: %s", expectedID)
		}
	})

	t.Run("cleanup with concurrent access", func(t *testing.T) {
		tempDir, err := os.MkdirTemp("", "padz_test_*")
		require.NoError(t, err)
		defer os.RemoveAll(tempDir)

		cfg := &config.Config{
			DataPath:   tempDir,
			FileSystem: filesystem.NewOSFileSystem(),
		}

		store, err := NewStoreWithConfig(cfg)
		require.NoError(t, err)

		// Add some scratches
		oldTime := time.Now().AddDate(0, 0, -40) // 40 days ago
		newTime := time.Now()

		oldScratches := []Scratch{
			{ID: "old1", Project: "test", Title: "Old 1", CreatedAt: oldTime},
			{ID: "old2", Project: "test", Title: "Old 2", CreatedAt: oldTime},
		}

		newScratches := []Scratch{
			{ID: "new1", Project: "test", Title: "New 1", CreatedAt: newTime},
			{ID: "new2", Project: "test", Title: "New 2", CreatedAt: newTime},
		}

		// Add all scratches
		allScratches := append(oldScratches, newScratches...)
		err = store.SaveScratchesAtomic(allScratches)
		require.NoError(t, err)

		// Create dummy files for the scratches
		scratchPath, _ := GetScratchPath()
		for _, s := range allScratches {
			filePath := filepath.Join(scratchPath, s.ID)
			err := os.WriteFile(filePath, []byte("test content"), 0644)
			require.NoError(t, err)
		}

		// Simulate cleanup removing old scratches
		var wg sync.WaitGroup
		wg.Add(2)

		// Goroutine 1: Cleanup old scratches
		go func() {
			defer wg.Done()
			time.Sleep(10 * time.Millisecond)
			err := store.SaveScratchesAtomic(newScratches)
			assert.NoError(t, err)
		}()

		// Goroutine 2: Try to add a new scratch concurrently
		go func() {
			defer wg.Done()
			newScratch := Scratch{
				ID:        "concurrent",
				Project:   "test",
				Title:     "Added during cleanup",
				CreatedAt: time.Now(),
			}
			err := store.AddScratchAtomic(newScratch)
			assert.NoError(t, err)
		}()

		wg.Wait()

		// Check final state
		finalScratches := store.GetScratches()

		// Should have either just the new scratches, or new scratches + concurrent one
		assert.GreaterOrEqual(t, len(finalScratches), 2)
		assert.LessOrEqual(t, len(finalScratches), 3)

		// Old scratches should not be present
		for _, s := range finalScratches {
			assert.NotEqual(t, "old1", s.ID)
			assert.NotEqual(t, "old2", s.ID)
		}
	})
}
