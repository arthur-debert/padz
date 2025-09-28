package v2

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/filesystem"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func setupTestStore(t *testing.T) (*Store, string) {
	tmpDir := t.TempDir()
	fs := filesystem.NewOSFileSystem()
	cfg := &config.Config{
		FileSystem:    fs,
		DataPath:      tmpDir,
		IsGlobalScope: false,
	}

	store, err := NewStoreWithConfig(cfg)
	require.NoError(t, err)
	require.NotNil(t, store)

	return store, tmpDir
}

func TestNewStore(t *testing.T) {
	store, tmpDir := setupTestStore(t)
	defer store.Close()

	// Verify store files were created
	storePath := filepath.Join(tmpDir, "scratch", storeFileName)
	_, err := os.Stat(storePath)
	assert.NoError(t, err)

	// Verify files directory was created
	filesPath := filepath.Join(tmpDir, "scratch", "files")
	info, err := os.Stat(filesPath)
	assert.NoError(t, err)
	assert.True(t, info.IsDir())
}

func TestBasicCRUD(t *testing.T) {
	store, _ := setupTestStore(t)
	defer store.Close()

	t.Run("AddScratch", func(t *testing.T) {
		scratch := Scratch{
			ID:        "test-id-1",
			Title:     "Test Scratch",
			Project:   "test-project",
			CreatedAt: time.Now(),
			UpdatedAt: time.Now(),
		}

		err := store.AddScratch(scratch)
		assert.NoError(t, err)

		// Verify it was added
		scratches := store.GetScratches()
		assert.Len(t, scratches, 1)
		assert.Equal(t, "Test Scratch", scratches[0].Title)
		assert.Equal(t, "test-project", scratches[0].Project)
		assert.False(t, scratches[0].IsPinned)
		assert.False(t, scratches[0].IsDeleted)
	})

	t.Run("GetScratches", func(t *testing.T) {
		// Add multiple scratches
		for i := 0; i < 3; i++ {
			scratch := Scratch{
				ID:        fmt.Sprintf("test-id-%d", i+2),
				Title:     fmt.Sprintf("Scratch %d", i+2),
				Project:   "test-project",
				CreatedAt: time.Now().Add(-time.Duration(i) * time.Hour),
				UpdatedAt: time.Now(),
			}
			err := store.AddScratch(scratch)
			require.NoError(t, err)
		}

		scratches := store.GetScratches()
		assert.Len(t, scratches, 4)

		// Verify they have simple IDs
		for i, s := range scratches {
			assert.NotEmpty(t, s.ID)
			assert.Contains(t, []string{"1", "2", "3", "4"}, s.ID)
			t.Logf("Scratch %d: ID=%s, Title=%s", i, s.ID, s.Title)
		}
	})

	t.Run("UpdateScratch", func(t *testing.T) {
		scratches := store.GetScratches()
		require.Greater(t, len(scratches), 0)

		// Update the first scratch
		scratch := scratches[0]
		scratch.Title = "Updated Title"
		scratch.Size = 1024

		err := store.UpdateScratch(scratch)
		assert.NoError(t, err)

		// Verify update
		updated := store.GetScratches()
		found := false
		for _, s := range updated {
			if s.ID == scratch.ID {
				assert.Equal(t, "Updated Title", s.Title)
				assert.Equal(t, int64(1024), s.Size)
				found = true
				break
			}
		}
		assert.True(t, found)
	})

	t.Run("RemoveScratch", func(t *testing.T) {
		scratches := store.GetScratches()
		initialCount := len(scratches)
		require.Greater(t, initialCount, 0)

		// Remove the first scratch
		err := store.RemoveScratch(scratches[0].ID)
		assert.NoError(t, err)

		// Verify it's soft deleted (not in active list)
		remaining := store.GetScratches()
		assert.Len(t, remaining, initialCount-1)

		// Verify the ID is not reused
		for _, s := range remaining {
			assert.NotEqual(t, scratches[0].ID, s.ID)
		}
	})
}

func TestPinning(t *testing.T) {
	store, _ := setupTestStore(t)
	defer store.Close()

	// Add scratches
	for i := 0; i < 3; i++ {
		scratch := Scratch{
			ID:        fmt.Sprintf("test-id-%d", i),
			Title:     fmt.Sprintf("Scratch %d", i),
			Project:   "test-project",
			CreatedAt: time.Now().Add(-time.Duration(i) * time.Hour),
			UpdatedAt: time.Now(),
		}
		err := store.AddScratch(scratch)
		require.NoError(t, err)
	}

	t.Run("PinScratch", func(t *testing.T) {
		scratches := store.GetScratches()
		require.Len(t, scratches, 3)

		// Pin the first scratch
		scratch := scratches[0]
		scratch.IsPinned = true
		scratch.PinnedAt = time.Now()

		err := store.UpdateScratch(scratch)
		assert.NoError(t, err)

		// Verify pinned scratches
		pinned := store.GetPinnedScratches()
		assert.Len(t, pinned, 1)
		assert.Equal(t, scratch.Title, pinned[0].Title)
		assert.True(t, pinned[0].IsPinned)
		assert.NotZero(t, pinned[0].PinnedAt)

		// Verify ID has pinned prefix
		assert.Equal(t, "p1", pinned[0].ID)
	})

	t.Run("MultiplePins", func(t *testing.T) {
		scratches := store.GetScratches()

		// Pin second scratch
		scratch2 := scratches[1]
		scratch2.IsPinned = true
		scratch2.PinnedAt = time.Now().Add(1 * time.Minute) // Newer pin

		err := store.UpdateScratch(scratch2)
		assert.NoError(t, err)

		// Verify pinned order (newer first)
		pinned := store.GetPinnedScratches()
		assert.Len(t, pinned, 2)
		assert.Equal(t, "p1", pinned[0].ID)
		assert.Equal(t, "p2", pinned[1].ID)
		assert.Equal(t, scratch2.Title, pinned[0].Title) // Newer pin is first
	})
}

func TestIDResolution(t *testing.T) {
	store, _ := setupTestStore(t)
	defer store.Close()

	// Add test scratches
	scratch1 := Scratch{
		ID:        "abc123",
		Title:     "First Scratch",
		Project:   "test",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	err := store.AddScratch(scratch1)
	require.NoError(t, err)

	scratch2 := Scratch{
		ID:        "def456",
		Title:     "Second Scratch",
		Project:   "test",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
		IsPinned:  true,
		PinnedAt:  time.Now(),
	}
	err = store.AddScratch(scratch2)
	require.NoError(t, err)

	t.Run("ResolveNumericID", func(t *testing.T) {
		// Should resolve "1" to first scratch's UUID
		uuid, err := store.resolveID("1")
		assert.NoError(t, err)
		assert.NotEmpty(t, uuid)
	})

	t.Run("ResolvePinnedID", func(t *testing.T) {
		// Should resolve "p1" to pinned scratch's UUID
		uuid, err := store.resolveID("p1")
		assert.NoError(t, err)
		assert.NotEmpty(t, uuid)
	})

	t.Run("ResolvePartialUUID", func(t *testing.T) {
		scratches := store.GetScratches()
		require.Greater(t, len(scratches), 0)

		// Get actual UUID from store
		allScratches, err := store.store.Query().StatusIn("active", "deleted").Find()
		require.NoError(t, err)
		require.Greater(t, len(allScratches), 0)

		actualUUID := allScratches[0].UUID
		partialUUID := actualUUID[:8]

		// Should resolve partial UUID
		uuid, err := store.resolveID(partialUUID)
		assert.NoError(t, err)
		assert.Equal(t, actualUUID, uuid)
	})
}
