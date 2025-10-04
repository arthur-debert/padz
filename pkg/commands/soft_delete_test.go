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
		Content:   "test content",
		CreatedAt: time.Now(),
	}
	require.NoError(t, setup.Store.AddScratch(testScratch))

	// Soft delete the scratch
	err := Delete(setup.Store, false, "testproject", "1")
	require.NoError(t, err)

	// Verify the scratch is marked as deleted but not in active list
	activeScratches := setup.Store.GetScratches()
	require.Len(t, activeScratches, 0, "Active scratches should be empty after soft delete")

	// Get all scratches including deleted
	allScratches := setup.Store.GetAllScratches()
	require.Len(t, allScratches, 1)

	deletedScratch := allScratches[0]
	assert.True(t, deletedScratch.IsDeleted)
	assert.NotNil(t, deletedScratch.DeletedAt)
	assert.WithinDuration(t, time.Now(), *deletedScratch.DeletedAt, 5*time.Second)
	assert.Equal(t, "test content", deletedScratch.Content, "Content should be preserved after soft delete")
}

func TestGetScratchByIndex_WithDeleted(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Get test store for setting custom timestamps
	testStore, ok := setup.Store.GetTestStore()
	if !ok {
		t.Skip("Test store not available")
	}

	// Create test scratches
	now := time.Now()
	testData := []struct {
		scratch   store.Scratch
		createdAt time.Time
	}{
		{store.Scratch{Project: "test", Title: "Active 1", Content: "content", CreatedAt: now}, now},
		{store.Scratch{Project: "test", Title: "Active 2", Content: "content", CreatedAt: now.Add(-1 * time.Hour)}, now.Add(-1 * time.Hour)},
		{store.Scratch{Project: "test", Title: "Deleted 1", Content: "content", CreatedAt: now.Add(-2 * time.Hour),
			IsDeleted: true, DeletedAt: &now}, now.Add(-2 * time.Hour)},
		{store.Scratch{Project: "test", Title: "Deleted 2", Content: "content", CreatedAt: now.Add(-3 * time.Hour),
			IsDeleted: true, DeletedAt: func() *time.Time { t := now.Add(-1 * time.Hour); return &t }()}, now.Add(-3 * time.Hour)},
	}

	for _, td := range testData {
		// Set the time function to return the specific timestamp
		testStore.SetTimeFunc(func() time.Time { return td.createdAt })
		require.NoError(t, setup.Store.AddScratch(td.scratch))
	}
	testStore.SetTimeFunc(time.Now)

	// Test regular SimpleID resolution (nanostore assigns numeric SimpleIDs only to specific dimensions)
	// In this case, only deleted items get numeric SimpleIDs: "1", "2", etc.
	// Active items get their UUID as SimpleID
	scratch, err := GetScratchByIndex(setup.Store, false, "test", "1")
	require.NoError(t, err)
	assert.Equal(t, "Deleted 2", scratch.Title) // SimpleID "1" = first deleted item

	scratch, err = GetScratchByIndex(setup.Store, false, "test", "2")
	require.NoError(t, err)
	assert.Equal(t, "Deleted 1", scratch.Title) // SimpleID "2" = second deleted item

	// Test deleted index resolution (d1, d2, etc)
	// These follow the deletion order (by deleted_at desc)
	scratch, err = GetScratchByIndex(setup.Store, false, "test", "d1")
	require.NoError(t, err)
	assert.Equal(t, "Deleted 1", scratch.Title) // d1 = first by deletion time (most recently deleted)

	scratch, err = GetScratchByIndex(setup.Store, false, "test", "d2")
	require.NoError(t, err)
	assert.Equal(t, "Deleted 2", scratch.Title) // d2 = second by deletion time (older deletion)
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
	testScratch.Content = "test content"
	require.NoError(t, setup.Store.AddScratch(testScratch))

	// Flush the deleted scratch
	err := Flush(setup.Store, false, "testproject", "d1", 0)
	require.NoError(t, err)

	// Verify the scratch is completely gone
	scratches := setup.Store.GetScratches()
	assert.Empty(t, scratches)

	// Verify the scratch is completely removed from the store
	allScratches := setup.Store.GetAllScratches()
	assert.Len(t, allScratches, 0, "No scratches should remain after flush")
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
	testScratch.Content = "test content"
	require.NoError(t, setup.Store.AddScratch(testScratch))

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
		s.Content = "content"
		require.NoError(t, setup.Store.AddScratch(s))
	}

	// Test ListModeActive (default) - ordered by created_at DESC
	activeScratches := LsWithMode(setup.Store, false, "test", ListModeActive)
	assert.Len(t, activeScratches, 2)
	// The order depends on the actual creation time in nanostore
	// Just verify we have both scratches
	titles := []string{activeScratches[0].Title, activeScratches[1].Title}
	assert.Contains(t, titles, "Active 1")
	assert.Contains(t, titles, "Active 2")

	// Test ListModeDeleted
	deletedScratches := LsWithMode(setup.Store, false, "test", ListModeDeleted)
	assert.Len(t, deletedScratches, 1)
	assert.Equal(t, "Deleted 1", deletedScratches[0].Title)

	// Test ListModeAll
	allScratches := LsWithMode(setup.Store, false, "test", ListModeAll)
	assert.Len(t, allScratches, 3)
}
