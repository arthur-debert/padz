package commands

import (
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestSoftDelete(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create a test scratch
	testScratch := store.Scratch{
		ID:        "test1",
		Project:   "testproject",
		Title:     "Test Scratch",
		CreatedAt: time.Now(),
	}
	require.NoError(t, setup.Store.AddScratch(testScratch))
	setup.WriteScratchFile(t, testScratch.ID, []byte("test content"))

	// Soft delete the scratch
	err := Delete(setup.Store, false, "testproject", "1")
	require.NoError(t, err)

	// Verify the scratch is marked as deleted
	scratches := setup.Store.GetScratches()
	require.Len(t, scratches, 1)

	deletedScratch := scratches[0]
	assert.True(t, deletedScratch.IsDeleted)
	assert.NotNil(t, deletedScratch.DeletedAt)
	assert.WithinDuration(t, time.Now(), *deletedScratch.DeletedAt, 5*time.Second)

	// Verify the file still exists
	path, _ := store.GetScratchFilePathWithConfig(testScratch.ID, setup.Config)
	_, err = setup.Config.FileSystem.Stat(path)
	assert.NoError(t, err, "File should still exist after soft delete")
}

func TestGetScratchByIndex_WithDeleted(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create test scratches
	now := time.Now()
	scratches := []store.Scratch{
		{ID: "active1", Project: "test", Title: "Active 1", CreatedAt: now},
		{ID: "active2", Project: "test", Title: "Active 2", CreatedAt: now.Add(-1 * time.Hour)},
		{ID: "deleted1", Project: "test", Title: "Deleted 1", CreatedAt: now.Add(-2 * time.Hour),
			IsDeleted: true, DeletedAt: &now},
		{ID: "deleted2", Project: "test", Title: "Deleted 2", CreatedAt: now.Add(-3 * time.Hour),
			IsDeleted: true, DeletedAt: func() *time.Time { t := now.Add(-1 * time.Hour); return &t }()},
	}

	for _, s := range scratches {
		require.NoError(t, setup.Store.AddScratch(s))
		setup.WriteScratchFile(t, s.ID, []byte("content"))
	}

	// Test regular index resolution (should exclude deleted)
	scratch, err := GetScratchByIndex(setup.Store, false, "test", "1")
	require.NoError(t, err)
	assert.Equal(t, "active1", scratch.ID)

	scratch, err = GetScratchByIndex(setup.Store, false, "test", "2")
	require.NoError(t, err)
	assert.Equal(t, "active2", scratch.ID)

	// Test deleted index resolution
	// Note: deleted items are sorted by DeletedAt descending (newest first)
	scratch, err = GetScratchByIndex(setup.Store, false, "test", "d1")
	require.NoError(t, err)
	assert.Equal(t, "deleted1", scratch.ID) // deleted1 has more recent DeletedAt

	scratch, err = GetScratchByIndex(setup.Store, false, "test", "d2")
	require.NoError(t, err)
	assert.Equal(t, "deleted2", scratch.ID) // deleted2 has older DeletedAt
}

func TestFlush(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create and soft-delete a scratch
	testScratch := store.Scratch{
		ID:        "test1",
		Project:   "testproject",
		Title:     "Test Scratch",
		CreatedAt: time.Now(),
		IsDeleted: true,
		DeletedAt: func() *time.Time { t := time.Now(); return &t }(),
	}
	require.NoError(t, setup.Store.AddScratch(testScratch))
	setup.WriteScratchFile(t, testScratch.ID, []byte("test content"))

	// Flush the deleted scratch
	err := Flush(setup.Store, false, "testproject", "d1", 0)
	require.NoError(t, err)

	// Verify the scratch is completely gone
	scratches := setup.Store.GetScratches()
	assert.Empty(t, scratches)

	// Verify the file is deleted
	path, _ := store.GetScratchFilePathWithConfig(testScratch.ID, setup.Config)
	_, err = setup.Config.FileSystem.Stat(path)
	assert.Error(t, err, "File should be deleted after flush")
}

func TestRestore(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create a soft-deleted scratch
	deletedAt := time.Now()
	testScratch := store.Scratch{
		ID:        "test1",
		Project:   "testproject",
		Title:     "Deleted Scratch",
		CreatedAt: time.Now().Add(-1 * time.Hour),
		IsDeleted: true,
		DeletedAt: &deletedAt,
	}
	require.NoError(t, setup.Store.AddScratch(testScratch))
	setup.WriteScratchFile(t, testScratch.ID, []byte("test content"))

	// Restore the scratch
	err := Restore(setup.Store, false, "testproject", "d1", 0)
	require.NoError(t, err)

	// Verify the scratch is restored
	scratches := setup.Store.GetScratches()
	require.Len(t, scratches, 1)

	restoredScratch := scratches[0]
	assert.False(t, restoredScratch.IsDeleted)
	assert.Nil(t, restoredScratch.DeletedAt)
}

func TestLsWithMode(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create mixed scratches
	now := time.Now()
	scratches := []store.Scratch{
		{ID: "active1", Project: "test", Title: "Active 1", CreatedAt: now},
		{ID: "deleted1", Project: "test", Title: "Deleted 1", CreatedAt: now.Add(-1 * time.Hour),
			IsDeleted: true, DeletedAt: &now},
		{ID: "active2", Project: "test", Title: "Active 2", CreatedAt: now.Add(-2 * time.Hour)},
	}

	for _, s := range scratches {
		require.NoError(t, setup.Store.AddScratch(s))
		setup.WriteScratchFile(t, s.ID, []byte("content"))
	}

	// Test ListModeActive (default)
	activeScratches := LsWithMode(setup.Store, false, "test", ListModeActive)
	assert.Len(t, activeScratches, 2)
	assert.Equal(t, "active1", activeScratches[0].ID)
	assert.Equal(t, "active2", activeScratches[1].ID)

	// Test ListModeDeleted
	deletedScratches := LsWithMode(setup.Store, false, "test", ListModeDeleted)
	assert.Len(t, deletedScratches, 1)
	assert.Equal(t, "deleted1", deletedScratches[0].ID)

	// Test ListModeAll
	allScratches := LsWithMode(setup.Store, false, "test", ListModeAll)
	assert.Len(t, allScratches, 3)
}
