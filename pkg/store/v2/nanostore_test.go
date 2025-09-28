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

func setupNanoTestStore(t *testing.T) (*NanoStore, string) {
	tmpDir := t.TempDir()
	fs := filesystem.NewOSFileSystem()
	cfg := &config.Config{
		FileSystem:    fs,
		DataPath:      tmpDir,
		IsGlobalScope: false,
	}

	store, err := NewNanoStoreWithConfig(cfg)
	require.NoError(t, err)
	require.NotNil(t, store)

	return store, tmpDir
}

func TestNanoStoreBasicOperations(t *testing.T) {
	store, tmpDir := setupNanoTestStore(t)
	defer func() {
		if err := store.Close(); err != nil {
			t.Errorf("Failed to close store: %v", err)
		}
	}()

	// Note: nanostore file is created on first write, not on initialization
	// So we skip checking for file existence here

	t.Run("AddAndGetScratch", func(t *testing.T) {
		scratch := Scratch{
			Title:     "Test Scratch",
			Project:   "test-project",
			CreatedAt: time.Now(),
			UpdatedAt: time.Now(),
		}

		// Add scratch
		err := store.AddScratch(scratch)
		assert.NoError(t, err)

		// Get all scratches
		scratches := store.GetScratches()
		assert.Len(t, scratches, 1)
		assert.Equal(t, "Test Scratch", scratches[0].Title)
		// TODO: Project needs to be stored as data, not dimension
		// assert.Equal(t, "test-project", scratches[0].Project)
		assert.Equal(t, "1", scratches[0].ID) // Should get SimpleID
		// UUID is not exposed in Scratch struct
	})

	t.Run("MultipleScratches", func(t *testing.T) {
		// Add more scratches
		for i := 2; i <= 4; i++ {
			scratch := Scratch{
				Title:     fmt.Sprintf("Scratch %d", i),
				Project:   "test-project",
				CreatedAt: time.Now(),
				UpdatedAt: time.Now(),
			}
			err := store.AddScratch(scratch)
			require.NoError(t, err)
		}

		// Get all scratches
		scratches := store.GetScratches()
		assert.Len(t, scratches, 4)

		// Verify SimpleIDs
		for i, s := range scratches {
			assert.Equal(t, fmt.Sprintf("%d", i+1), s.ID)
			t.Logf("Scratch %d: ID=%s, Title=%s", i, s.ID, s.Title)
		}
	})

	t.Run("PinScratch", func(t *testing.T) {
		scratches := store.GetScratches()
		require.Greater(t, len(scratches), 0)

		// Pin the first scratch
		scratch := scratches[0]
		scratch.IsPinned = true
		scratch.PinnedAt = time.Now()

		err := store.UpdateScratch(scratch)
		assert.NoError(t, err)

		// Get pinned scratches
		pinned := store.GetPinnedScratches()
		assert.Len(t, pinned, 1)
		assert.Equal(t, "p1", pinned[0].ID) // Should have pinned prefix
		assert.True(t, pinned[0].IsPinned)
	})

	t.Run("RemoveScratch", func(t *testing.T) {
		scratches := store.GetScratches()
		initialCount := len(scratches)
		require.Greater(t, initialCount, 0)

		// Remove a scratch
		err := store.RemoveScratch(scratches[0].ID)
		assert.NoError(t, err)

		// Verify it's soft deleted
		remaining := store.GetScratches()
		assert.Len(t, remaining, initialCount-1)
	})

	t.Run("SearchScratches", func(t *testing.T) {
		// Add a scratch with unique title
		scratch := Scratch{
			Title:     "Unique Search Term",
			Project:   "test-project",
			CreatedAt: time.Now(),
			UpdatedAt: time.Now(),
		}
		err := store.AddScratch(scratch)
		require.NoError(t, err)

		// Search for it
		results := store.Search("Unique")
		assert.Greater(t, len(results), 0)

		found := false
		for _, r := range results {
			if r.Title == "Unique Search Term" {
				found = true
				break
			}
		}
		assert.True(t, found, "Should find scratch with search term")
	})
}

func TestNanoStoreIDResolution(t *testing.T) {
	store, _ := setupNanoTestStore(t)
	defer func() {
		if err := store.Close(); err != nil {
			t.Errorf("Failed to close store: %v", err)
		}
	}()

	// Add test scratches
	scratch1 := Scratch{
		Title:     "First",
		Project:   "test",
		CreatedAt: time.Now(),
	}
	err := store.AddScratch(scratch1)
	require.NoError(t, err)

	scratch2 := Scratch{
		Title:     "Second",
		Project:   "test",
		CreatedAt: time.Now(),
		IsPinned:  true,
		PinnedAt:  time.Now(),
	}
	err = store.AddScratch(scratch2)
	require.NoError(t, err)

	// Get all scratches to verify IDs
	scratches := store.GetScratches()
	for _, s := range scratches {
		t.Logf("Scratch: ID=%s, Title=%s, IsPinned=%v", s.ID, s.Title, s.IsPinned)
	}

	t.Run("ResolveNumericID", func(t *testing.T) {
		uuid, err := store.store.ResolveUUID("1")
		assert.NoError(t, err)
		assert.NotEmpty(t, uuid)
	})

	t.Run("ResolvePinnedID", func(t *testing.T) {
		uuid, err := store.store.ResolveUUID("p1")
		assert.NoError(t, err)
		assert.NotEmpty(t, uuid)
	})

	t.Run("ResolvePartialUUID", func(t *testing.T) {
		// Get a UUID to test with
		scratches := store.GetScratches()
		require.Greater(t, len(scratches), 0)

		// Get UUID by resolving SimpleID
		uuid, err := store.store.ResolveUUID(scratches[0].ID)
		require.NoError(t, err)
		require.NotEmpty(t, uuid)

		// Test partial resolution
		if len(uuid) >= 8 {
			partial := uuid[:8]
			resolved, err := store.store.ResolveUUID(partial)
			assert.NoError(t, err)
			assert.Equal(t, uuid, resolved)
		}
	})
}
