package commands

import (
	"fmt"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/arthur-debert/padz/pkg/testutil"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestPin(t *testing.T) {
	t.Run("pin a scratch successfully", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create a scratch with unique content
		uniqueContent := fmt.Sprintf("content-%s", time.Now().Format("20060102150405.000000"))
		scratch := store.Scratch{
			Project:   project,
			Title:     "Test Scratch",
			Content:   uniqueContent,
			CreatedAt: time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Pin it by index 1 (should be the only scratch)
		err = Pin(st, false, project, "1")
		assert.NoError(t, err)

		// Verify it's pinned by finding it via content
		scratches := st.GetScratches()
		found := false
		for _, s := range scratches {
			if s.Content == uniqueContent {
				assert.True(t, s.IsPinned)
				assert.NotZero(t, s.PinnedAt)
				found = true
				break
			}
		}
		assert.True(t, found, "scratch with unique content not found")
	})

	t.Run("error when scratch already pinned", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create and add a pinned scratch with unique content
		uniqueContent := fmt.Sprintf("pinned-content-%s", time.Now().Format("20060102150405.000000"))
		scratch := store.Scratch{
			Project:   project,
			Title:     "Test Scratch",
			Content:   uniqueContent,
			CreatedAt: time.Now(),
			IsPinned:  true,
			PinnedAt:  time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Get the actual ID assigned
		scratches := st.GetPinnedScratches()
		require.Len(t, scratches, 1)
		assignedID := scratches[0].ID

		// Try to pin it again using its actual ID
		err = Pin(st, false, project, assignedID)
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "already pinned")
	})

	t.Run("error when max pinned scratches reached", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create max pinned scratches with unique content
		for i := 0; i < store.MaxPinnedScratches; i++ {
			scratch := store.Scratch{
				Project:   project,
				Title:     fmt.Sprintf("Test Scratch %d", i),
				Content:   fmt.Sprintf("pinned-content-%d-%s", i, time.Now().Format("20060102150405")),
				CreatedAt: time.Now().Add(-time.Duration(i) * time.Hour),
				IsPinned:  true,
				PinnedAt:  time.Now(),
			}
			err = st.AddScratch(scratch)
			require.NoError(t, err)
		}

		// Create one more unpinned with unique content
		unpinnedContent := fmt.Sprintf("unpinned-content-%s", time.Now().Format("20060102150405.000000"))
		scratch := store.Scratch{
			Project:   project,
			Title:     "Unpinned Scratch",
			Content:   unpinnedContent,
			CreatedAt: time.Now().Add(-time.Duration(store.MaxPinnedScratches+1) * time.Hour),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Try to pin it (unpinned items have their own sequential numbering starting from 1)
		err = Pin(st, false, project, "1")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "maximum number of pinned scratches")
	})

	t.Run("pin by index", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create a scratch with unique content
		uniqueContent := fmt.Sprintf("index-pin-content-%s", time.Now().Format("20060102150405.000000"))
		scratch := store.Scratch{
			Project:   project,
			Title:     "Test Scratch",
			Content:   uniqueContent,
			CreatedAt: time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Pin by index
		err = Pin(st, false, project, "1")
		assert.NoError(t, err)

		// Verify it's pinned by finding it via content
		scratches := st.GetScratches()
		found := false
		for _, s := range scratches {
			if s.Content == uniqueContent {
				assert.True(t, s.IsPinned)
				found = true
				break
			}
		}
		assert.True(t, found, "scratch with unique content not found")
	})
}

func TestUnpin(t *testing.T) {
	t.Run("unpin a scratch successfully", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create a pinned scratch with unique content
		uniqueContent := fmt.Sprintf("unpin-content-%s", time.Now().Format("20060102150405.000000"))
		scratch := store.Scratch{
			Project:   project,
			Title:     "Test Scratch",
			Content:   uniqueContent,
			CreatedAt: time.Now(),
			IsPinned:  true,
			PinnedAt:  time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Get the actual assigned ID
		scratches := st.GetPinnedScratches()
		require.Len(t, scratches, 1)
		assignedID := scratches[0].ID

		// Unpin it using its actual ID
		err = Unpin(st, false, project, assignedID)
		assert.NoError(t, err)

		// Verify it's unpinned by finding it via content
		scratches = st.GetScratches()
		found := false
		for _, s := range scratches {
			if s.Content == uniqueContent {
				assert.False(t, s.IsPinned)
				assert.Zero(t, s.PinnedAt)
				found = true
				break
			}
		}
		assert.True(t, found, "scratch with unique content not found")
	})

	t.Run("error when scratch not pinned", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create an unpinned scratch with unique content
		uniqueContent := fmt.Sprintf("unpinned-error-content-%s", time.Now().Format("20060102150405.000000"))
		scratch := store.Scratch{
			Project:   project,
			Title:     "Test Scratch",
			Content:   uniqueContent,
			CreatedAt: time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Get the actual assigned ID
		scratches := st.GetScratches()
		require.Len(t, scratches, 1)
		assignedID := scratches[0].ID

		// Try to unpin it
		err = Unpin(st, false, project, assignedID)
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "not pinned")
	})

	t.Run("unpin by pinned index", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create multiple scratches, some pinned with unique content
		pinned1Content := fmt.Sprintf("pinned1-content-%s", time.Now().Format("20060102150405.000000"))
		pinned2Content := fmt.Sprintf("pinned2-content-%s", time.Now().Format("20060102150405.000000"))
		unpinnedContent := fmt.Sprintf("unpinned-content-%s", time.Now().Format("20060102150405.000000"))

		scratches := []store.Scratch{
			{
				Project:   project,
				Title:     "Pinned 1",
				Content:   pinned1Content,
				CreatedAt: time.Now().Add(-1 * time.Hour),
				IsPinned:  true,
				PinnedAt:  time.Now().Add(-10 * time.Minute),
			},
			{
				Project:   project,
				Title:     "Pinned 2",
				Content:   pinned2Content,
				CreatedAt: time.Now().Add(-2 * time.Hour),
				IsPinned:  true,
				PinnedAt:  time.Now().Add(-5 * time.Minute), // Newer pinned time
			},
			{
				Project:   project,
				Title:     "Unpinned",
				Content:   unpinnedContent,
				CreatedAt: time.Now(),
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Unpin using p1 (nanostore assigns p1 to the first pinned item in the list)
		err = Unpin(st, false, project, "p1")
		assert.NoError(t, err)

		// Verify correct scratch was unpinned by checking content
		updated := st.GetScratches()
		pinned1Found := false
		pinned2Found := false
		for _, s := range updated {
			if s.Content == pinned1Content {
				pinned1Found = true
				// One of the pinned items should be unpinned
			}
			if s.Content == pinned2Content {
				pinned2Found = true
				// One should still be pinned
			}
		}
		assert.True(t, pinned1Found, "pinned1 not found")
		assert.True(t, pinned2Found, "pinned2 not found")

		// Verify we now have only 1 pinned item
		pinnedScratches := st.GetPinnedScratches()
		assert.Len(t, pinnedScratches, 1)
	})
}

func TestGetScratchByIndex_PinnedIndices(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	st, err := store.NewStoreWithConfig(cfg)
	require.NoError(t, err)
	project := "test-project"

	// Get test store for custom timestamps
	testStore, ok := st.GetTestStore()
	if !ok {
		t.Skip("Test store not available")
	}

	// Create scratches with specific order
	now := time.Now()
	testData := []struct {
		scratch   store.Scratch
		createdAt time.Time
	}{
		{
			scratch: store.Scratch{
				Project:   project,
				Title:     "Newest",
				Content:   "content",
				CreatedAt: now,
			},
			createdAt: now,
		},
		{
			scratch: store.Scratch{
				Project:   project,
				Title:     "Old but pinned",
				Content:   "content",
				CreatedAt: now.Add(-48 * time.Hour),
				IsPinned:  true,
				PinnedAt:  now.Add(-10 * time.Minute),
			},
			createdAt: now.Add(-48 * time.Hour),
		},
		{
			scratch: store.Scratch{
				Project:   project,
				Title:     "Pinned newer",
				Content:   "content",
				CreatedAt: now.Add(-24 * time.Hour),
				IsPinned:  true,
				PinnedAt:  now.Add(-5 * time.Minute),
			},
			createdAt: now.Add(-24 * time.Hour),
		},
		{
			scratch: store.Scratch{
				Project:   project,
				Title:     "Middle",
				Content:   "content",
				CreatedAt: now.Add(-12 * time.Hour),
			},
			createdAt: now.Add(-12 * time.Hour),
		},
	}

	for _, td := range testData {
		// Set the time function to return the specific timestamp
		testStore.SetTimeFunc(func() time.Time { return td.createdAt })
		err = st.AddScratch(td.scratch)
		require.NoError(t, err)
	}
	testStore.SetTimeFunc(time.Now)

	// Test pinned indices (sorted by PinnedAt, newest first)
	t.Run("p1 returns first pinned", func(t *testing.T) {
		scratch, err := GetScratchByIndex(st, false, project, "p1")
		assert.NoError(t, err)
		assert.Equal(t, "Pinned newer", scratch.Title) // Most recently pinned
	})

	t.Run("p2 returns second pinned", func(t *testing.T) {
		scratch, err := GetScratchByIndex(st, false, project, "p2")
		assert.NoError(t, err)
		assert.Equal(t, "Old but pinned", scratch.Title) // Second most recently pinned
	})

	t.Run("regular index 1 returns first in chronological order", func(t *testing.T) {
		scratch, err := GetScratchByIndex(st, false, project, "1")
		assert.NoError(t, err)
		assert.Equal(t, "Newest", scratch.Title) // First by creation time (newest)
	})

	t.Run("regular index 4 returns oldest", func(t *testing.T) {
		// Note: With only 2 unpinned scratches (Newest and Middle), index 4 is out of range
		// Let's test index 2 instead
		scratch, err := GetScratchByIndex(st, false, project, "2")
		assert.NoError(t, err)
		assert.Equal(t, "Middle", scratch.Title) // Second unpinned scratch
	})

	t.Run("invalid pinned index", func(t *testing.T) {
		_, err := GetScratchByIndex(st, false, project, "p3")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "pinned index out of range")
	})
}

func TestLs_WithPinned(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	st, err := store.NewStoreWithConfig(cfg)
	require.NoError(t, err)
	project := "test-project"

	// Get test store for custom timestamps
	testStore, ok := st.GetTestStore()
	if !ok {
		t.Skip("Test store not available")
	}

	// Create scratches with unique content
	now := time.Now()
	newestContent := fmt.Sprintf("newest-content-%s", now.Format("20060102150405.000000"))
	pinned1Content := fmt.Sprintf("pinned1-content-%s", now.Format("20060102150405.000000"))
	middleContent := fmt.Sprintf("middle-content-%s", now.Format("20060102150405.000000"))
	pinned2Content := fmt.Sprintf("pinned2-content-%s", now.Format("20060102150405.000000"))

	testData := []struct {
		scratch   store.Scratch
		createdAt time.Time
	}{
		{
			scratch: store.Scratch{
				Project:   project,
				Title:     "Newest",
				Content:   newestContent,
				CreatedAt: now,
			},
			createdAt: now,
		},
		{
			scratch: store.Scratch{
				Project:   project,
				Title:     "Pinned 1",
				Content:   pinned1Content,
				CreatedAt: now.Add(-48 * time.Hour),
				IsPinned:  true,
				PinnedAt:  now.Add(-5 * time.Minute),
			},
			createdAt: now.Add(-48 * time.Hour),
		},
		{
			scratch: store.Scratch{
				Project:   project,
				Title:     "Middle",
				Content:   middleContent,
				CreatedAt: now.Add(-24 * time.Hour),
			},
			createdAt: now.Add(-24 * time.Hour),
		},
		{
			scratch: store.Scratch{
				Project:   project,
				Title:     "Pinned 2",
				Content:   pinned2Content,
				CreatedAt: now.Add(-72 * time.Hour),
				IsPinned:  true,
				PinnedAt:  now.Add(-10 * time.Minute),
			},
			createdAt: now.Add(-72 * time.Hour),
		},
	}

	for _, td := range testData {
		// Set the time function to return the specific timestamp
		testStore.SetTimeFunc(func() time.Time { return td.createdAt })
		err = st.AddScratch(td.scratch)
		require.NoError(t, err)
	}
	testStore.SetTimeFunc(time.Now)

	// Get sorted list
	result := Ls(st, false, project)

	// Verify order: chronological (by CreatedAt), pinned status doesn't affect order
	assert.Equal(t, 4, len(result))

	// Map content to expected order based on creation times (newest first)
	expectedOrder := []string{newestContent, middleContent, pinned1Content, pinned2Content}
	actualOrder := []string{result[0].Content, result[1].Content, result[2].Content, result[3].Content}

	assert.Equal(t, expectedOrder, actualOrder, "Scratches not in expected chronological order")
}

func TestPinMultiple(t *testing.T) {
	t.Run("pin multiple scratches successfully", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create multiple scratches
		scratches := []store.Scratch{
			{
				Project:   project,
				Title:     "Test 1",
				Content:   "test content",
				CreatedAt: time.Now(),
			},
			{
				Project:   project,
				Title:     "Test 2",
				Content:   "test content",
				CreatedAt: time.Now().Add(-1 * time.Hour),
			},
			{
				Project:   project,
				Title:     "Test 3",
				Content:   "test content",
				CreatedAt: time.Now().Add(-2 * time.Hour),
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Pin multiple by index
		pinnedTitles, err := PinMultiple(st, false, project, []string{"1", "3"})
		assert.NoError(t, err)
		assert.Equal(t, 2, len(pinnedTitles))
		assert.Contains(t, pinnedTitles, "Test 1")
		assert.Contains(t, pinnedTitles, "Test 3")

		// Verify they're pinned
		updatedScratches := st.GetScratches()
		for _, s := range updatedScratches {
			if s.Title == "Test 1" || s.Title == "Test 3" {
				assert.True(t, s.IsPinned)
				assert.NotZero(t, s.PinnedAt)
			} else {
				assert.False(t, s.IsPinned)
			}
		}
	})

	t.Run("skip already pinned scratches", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create scratches with one already pinned
		pinnedContent := fmt.Sprintf("already-pinned-%s", time.Now().Format("20060102150405.000000"))
		unpinnedContent := fmt.Sprintf("not-pinned-%s", time.Now().Format("20060102150405.000000"))

		scratches := []store.Scratch{
			{
				Project:   project,
				Title:     "Test 1",
				Content:   pinnedContent,
				CreatedAt: time.Now(),
				IsPinned:  true,
				PinnedAt:  time.Now(),
			},
			{
				Project:   project,
				Title:     "Test 2",
				Content:   unpinnedContent,
				CreatedAt: time.Now().Add(-1 * time.Hour),
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Get the actual IDs
		scratches = st.GetScratches()
		assert.Len(t, scratches, 2)

		// Try to pin both using their actual IDs
		ids := []string{scratches[0].ID, scratches[1].ID}
		pinnedTitles, err := PinMultiple(st, false, project, ids)
		assert.NoError(t, err)
		assert.Equal(t, 1, len(pinnedTitles)) // Only one newly pinned
		assert.Contains(t, pinnedTitles, "Test 2")
	})

	t.Run("error when exceeding pin limit", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create scratches - MaxPinnedScratches already pinned, plus extras
		for i := 0; i < store.MaxPinnedScratches+3; i++ {
			scratch := store.Scratch{
				Project:   project,
				Title:     fmt.Sprintf("Test %d", i),
				Content:   "test content",
				CreatedAt: time.Now().Add(-time.Duration(i) * time.Hour),
			}
			// Pin the first MaxPinnedScratches-1 (leave one slot)
			if i < store.MaxPinnedScratches-1 {
				scratch.IsPinned = true
				scratch.PinnedAt = time.Now()
			}
			err = st.AddScratch(scratch)
			require.NoError(t, err)
		}

		// Try to pin 2 more unpinned items (should fail as only 1 slot available)
		// The unpinned items will have indexes starting from 1
		ids := []string{"1", "2"}
		_, err = PinMultiple(st, false, project, ids)
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "only 1 slots available")
	})

	t.Run("handle duplicate IDs", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create a scratch
		scratch := store.Scratch{
			Project:   project,
			Title:     "Test 1",
			Content:   "test content",
			CreatedAt: time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Pin with duplicate IDs
		pinnedTitles, err := PinMultiple(st, false, project, []string{"1", "1", "1"})
		assert.NoError(t, err)
		assert.Equal(t, 1, len(pinnedTitles)) // Only one unique scratch pinned
		assert.Contains(t, pinnedTitles, "Test 1")
	})
}

func TestUnpinMultiple(t *testing.T) {
	t.Run("unpin multiple scratches successfully", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create multiple pinned scratches
		scratches := []store.Scratch{
			{
				ID:        "test1",
				Project:   project,
				Title:     "Test 1",
				CreatedAt: time.Now(),
				IsPinned:  true,
				PinnedAt:  time.Now(),
			},
			{
				ID:        "test2",
				Project:   project,
				Title:     "Test 2",
				CreatedAt: time.Now().Add(-1 * time.Hour),
				IsPinned:  true,
				PinnedAt:  time.Now(),
			},
			{
				ID:        "test3",
				Project:   project,
				Title:     "Test 3",
				CreatedAt: time.Now().Add(-2 * time.Hour),
				IsPinned:  true,
				PinnedAt:  time.Now(),
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Unpin multiple
		unpinnedTitles, err := UnpinMultiple(st, false, project, []string{"p1", "p3"})
		assert.NoError(t, err)
		assert.Equal(t, 2, len(unpinnedTitles))
		assert.Contains(t, unpinnedTitles, "Test 1")
		assert.Contains(t, unpinnedTitles, "Test 3")

		// Verify they're unpinned
		updatedScratches := st.GetScratches()
		for _, s := range updatedScratches {
			switch s.ID {
			case "test1", "test3":
				assert.False(t, s.IsPinned)
				assert.Zero(t, s.PinnedAt)
			case "test2":
				assert.True(t, s.IsPinned) // This one should still be pinned
			}
		}
	})

	t.Run("skip non-pinned scratches", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create scratches with one not pinned
		pinnedContent := fmt.Sprintf("skip-pinned-%s", time.Now().Format("20060102150405.000000"))
		unpinnedContent := fmt.Sprintf("skip-unpinned-%s", time.Now().Format("20060102150405.000000"))

		scratches := []store.Scratch{
			{
				Project:   project,
				Title:     "Test 1",
				Content:   pinnedContent,
				CreatedAt: time.Now(),
				IsPinned:  true,
				PinnedAt:  time.Now(),
			},
			{
				Project:   project,
				Title:     "Test 2",
				Content:   unpinnedContent,
				CreatedAt: time.Now().Add(-1 * time.Hour),
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Get the actual IDs
		scratches = st.GetScratches()
		assert.Len(t, scratches, 2)

		// Try to unpin both using their actual IDs
		ids := []string{scratches[0].ID, scratches[1].ID}
		unpinnedTitles, err := UnpinMultiple(st, false, project, ids)
		assert.NoError(t, err)
		assert.Equal(t, 1, len(unpinnedTitles)) // Only one was actually pinned
		assert.Contains(t, unpinnedTitles, "Test 1")
	})

	t.Run("unpin by mixed indices", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create scratches with unique content
		pinned1Content := fmt.Sprintf("mixed-pinned1-%s", time.Now().Format("20060102150405.000000"))
		pinned2Content := fmt.Sprintf("mixed-pinned2-%s", time.Now().Format("20060102150405.000000"))
		unpinnedContent := fmt.Sprintf("mixed-unpinned-%s", time.Now().Format("20060102150405.000000"))

		scratches := []store.Scratch{
			{
				Project:   project,
				Title:     "Test 1",
				Content:   pinned1Content,
				CreatedAt: time.Now(),
				IsPinned:  true,
				PinnedAt:  time.Now(),
			},
			{
				Project:   project,
				Title:     "Test 2",
				Content:   pinned2Content,
				CreatedAt: time.Now().Add(-1 * time.Hour),
				IsPinned:  true,
				PinnedAt:  time.Now(),
			},
			{
				Project:   project,
				Title:     "Test 3",
				Content:   unpinnedContent,
				CreatedAt: time.Now().Add(-2 * time.Hour),
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Get the actual IDs
		pinnedScratches := st.GetPinnedScratches()
		assert.Len(t, pinnedScratches, 2)

		// Find the scratch IDs by content
		var test1ID string
		for _, s := range pinnedScratches {
			if s.Content == pinned1Content {
				test1ID = s.ID
			}
		}

		// Unpin using mixed indices (one by ID, one by pinned index)
		// We'll use the actual ID for test1 and "p2" for the second pinned item
		unpinnedTitles, err := UnpinMultiple(st, false, project, []string{test1ID, "p2"})
		assert.NoError(t, err)
		assert.Equal(t, 2, len(unpinnedTitles))
		assert.Contains(t, unpinnedTitles, "Test 1")
		assert.Contains(t, unpinnedTitles, "Test 2")
	})
}
