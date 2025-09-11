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

		// Create a scratch
		scratch := store.Scratch{
			ID:        "test123",
			Project:   project,
			Title:     "Test Scratch",
			CreatedAt: time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Pin it
		err = Pin(st, false, false, project, "1")
		assert.NoError(t, err)

		// Verify it's pinned
		scratches := st.GetScratches()
		assert.True(t, scratches[0].IsPinned)
		assert.NotZero(t, scratches[0].PinnedAt)
	})

	t.Run("error when scratch already pinned", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create and add a pinned scratch
		scratch := store.Scratch{
			ID:        "test123",
			Project:   project,
			Title:     "Test Scratch",
			CreatedAt: time.Now(),
			IsPinned:  true,
			PinnedAt:  time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Try to pin it again
		err = Pin(st, false, false, project, "1")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "already pinned")
	})

	t.Run("error when max pinned scratches reached", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create max pinned scratches
		for i := 0; i < store.MaxPinnedScratches; i++ {
			scratch := store.Scratch{
				ID:        fmt.Sprintf("test%d", i),
				Project:   project,
				Title:     "Test Scratch",
				CreatedAt: time.Now().Add(-time.Duration(i) * time.Hour),
				IsPinned:  true,
				PinnedAt:  time.Now(),
			}
			err = st.AddScratch(scratch)
			require.NoError(t, err)
		}

		// Create one more unpinned
		scratch := store.Scratch{
			ID:        "test-unpinned",
			Project:   project,
			Title:     "Test Scratch",
			CreatedAt: time.Now().Add(-time.Duration(store.MaxPinnedScratches+1) * time.Hour),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Try to pin it
		err = Pin(st, false, false, project, fmt.Sprintf("%d", store.MaxPinnedScratches+1))
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "maximum number of pinned scratches")
	})

	t.Run("pin by hash ID", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create a scratch
		scratch := store.Scratch{
			ID:        "abc123def456",
			Project:   project,
			Title:     "Test Scratch",
			CreatedAt: time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Pin by partial hash
		err = Pin(st, false, false, project, "abc123")
		assert.NoError(t, err)

		// Verify it's pinned
		scratches := st.GetScratches()
		assert.True(t, scratches[0].IsPinned)
	})
}

func TestUnpin(t *testing.T) {
	t.Run("unpin a scratch successfully", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create a pinned scratch
		scratch := store.Scratch{
			ID:        "test123",
			Project:   project,
			Title:     "Test Scratch",
			CreatedAt: time.Now(),
			IsPinned:  true,
			PinnedAt:  time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Unpin it
		err = Unpin(st, false, false, project, "1")
		assert.NoError(t, err)

		// Verify it's unpinned
		scratches := st.GetScratches()
		assert.False(t, scratches[0].IsPinned)
		assert.Zero(t, scratches[0].PinnedAt)
	})

	t.Run("error when scratch not pinned", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create an unpinned scratch
		scratch := store.Scratch{
			ID:        "test123",
			Project:   project,
			Title:     "Test Scratch",
			CreatedAt: time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Try to unpin it
		err = Unpin(st, false, false, project, "1")
		assert.Error(t, err)
		assert.Contains(t, err.Error(), "not pinned")
	})

	t.Run("unpin by pinned index", func(t *testing.T) {
		cfg, cleanup := testutil.SetupTestEnvironment(t)
		defer cleanup()

		st, err := store.NewStoreWithConfig(cfg)
		require.NoError(t, err)
		project := "test-project"

		// Create multiple scratches, some pinned
		scratches := []store.Scratch{
			{
				ID:        "pinned1",
				Project:   project,
				Title:     "Pinned 1",
				CreatedAt: time.Now().Add(-1 * time.Hour),
				IsPinned:  true,
				PinnedAt:  time.Now().Add(-10 * time.Minute),
			},
			{
				ID:        "pinned2",
				Project:   project,
				Title:     "Pinned 2",
				CreatedAt: time.Now().Add(-2 * time.Hour),
				IsPinned:  true,
				PinnedAt:  time.Now().Add(-5 * time.Minute), // Newer pinned time
			},
			{
				ID:        "unpinned",
				Project:   project,
				Title:     "Unpinned",
				CreatedAt: time.Now(),
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Unpin using p1 (should be pinned1 as it appears first in chronological order)
		err = Unpin(st, false, false, project, "p1")
		assert.NoError(t, err)

		// Verify correct scratch was unpinned
		updated := st.GetScratches()
		for _, s := range updated {
			if s.ID == "pinned1" {
				assert.False(t, s.IsPinned)
			}
			if s.ID == "pinned2" {
				assert.True(t, s.IsPinned) // This one should still be pinned
			}
		}
	})
}

func TestGetScratchByIndex_PinnedIndices(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	st, err := store.NewStoreWithConfig(cfg)
	require.NoError(t, err)
	project := "test-project"

	// Create scratches with specific order
	scratches := []store.Scratch{
		{
			ID:        "newest",
			Project:   project,
			Title:     "Newest",
			CreatedAt: time.Now(),
		},
		{
			ID:        "pinned-old",
			Project:   project,
			Title:     "Old but pinned",
			CreatedAt: time.Now().Add(-48 * time.Hour),
			IsPinned:  true,
			PinnedAt:  time.Now().Add(-10 * time.Minute),
		},
		{
			ID:        "pinned-newer",
			Project:   project,
			Title:     "Pinned newer",
			CreatedAt: time.Now().Add(-24 * time.Hour),
			IsPinned:  true,
			PinnedAt:  time.Now().Add(-5 * time.Minute),
		},
		{
			ID:        "middle",
			Project:   project,
			Title:     "Middle",
			CreatedAt: time.Now().Add(-12 * time.Hour),
		},
	}

	for _, s := range scratches {
		err = st.AddScratch(s)
		require.NoError(t, err)
	}

	// Test pinned indices (in chronological order by CreatedAt)
	t.Run("p1 returns first pinned", func(t *testing.T) {
		scratch, err := GetScratchByIndex(st, false, false, project, "p1")
		assert.NoError(t, err)
		assert.Equal(t, "pinned-newer", scratch.ID) // First pinned in chronological order
	})

	t.Run("p2 returns second pinned", func(t *testing.T) {
		scratch, err := GetScratchByIndex(st, false, false, project, "p2")
		assert.NoError(t, err)
		assert.Equal(t, "pinned-old", scratch.ID) // Second pinned in chronological order
	})

	t.Run("regular index 1 returns first in chronological order", func(t *testing.T) {
		scratch, err := GetScratchByIndex(st, false, false, project, "1")
		assert.NoError(t, err)
		assert.Equal(t, "newest", scratch.ID) // First by creation time (newest)
	})

	t.Run("regular index 4 returns oldest", func(t *testing.T) {
		scratch, err := GetScratchByIndex(st, false, false, project, "4")
		assert.NoError(t, err)
		assert.Equal(t, "pinned-old", scratch.ID) // Last by creation time (oldest)
	})

	t.Run("invalid pinned index", func(t *testing.T) {
		_, err := GetScratchByIndex(st, false, false, project, "p3")
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

	// Create scratches
	now := time.Now()
	scratches := []store.Scratch{
		{
			ID:        "newest",
			Project:   project,
			Title:     "Newest",
			CreatedAt: now,
		},
		{
			ID:        "pinned1",
			Project:   project,
			Title:     "Pinned 1",
			CreatedAt: now.Add(-48 * time.Hour),
			IsPinned:  true,
			PinnedAt:  now.Add(-5 * time.Minute),
		},
		{
			ID:        "middle",
			Project:   project,
			Title:     "Middle",
			CreatedAt: now.Add(-24 * time.Hour),
		},
		{
			ID:        "pinned2",
			Project:   project,
			Title:     "Pinned 2",
			CreatedAt: now.Add(-72 * time.Hour),
			IsPinned:  true,
			PinnedAt:  now.Add(-10 * time.Minute),
		},
	}

	for _, s := range scratches {
		err = st.AddScratch(s)
		require.NoError(t, err)
	}

	// Get sorted list
	result := Ls(st, false, false, project)

	// Verify order: chronological (by CreatedAt), pinned status doesn't affect order
	assert.Equal(t, 4, len(result))
	assert.Equal(t, "newest", result[0].ID)  // Newest CreatedAt
	assert.Equal(t, "middle", result[1].ID)  // 24 hours ago
	assert.Equal(t, "pinned1", result[2].ID) // 48 hours ago (pinned)
	assert.Equal(t, "pinned2", result[3].ID) // 72 hours ago (pinned)
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
				ID:        "test1",
				Project:   project,
				Title:     "Test 1",
				CreatedAt: time.Now(),
			},
			{
				ID:        "test2",
				Project:   project,
				Title:     "Test 2",
				CreatedAt: time.Now().Add(-1 * time.Hour),
			},
			{
				ID:        "test3",
				Project:   project,
				Title:     "Test 3",
				CreatedAt: time.Now().Add(-2 * time.Hour),
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Pin multiple by index
		pinnedTitles, err := PinMultiple(st, false, false, project, []string{"1", "3"})
		assert.NoError(t, err)
		assert.Equal(t, 2, len(pinnedTitles))
		assert.Contains(t, pinnedTitles, "Test 1")
		assert.Contains(t, pinnedTitles, "Test 3")

		// Verify they're pinned
		updatedScratches := st.GetScratches()
		for _, s := range updatedScratches {
			if s.ID == "test1" || s.ID == "test3" {
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
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Try to pin both
		pinnedTitles, err := PinMultiple(st, false, false, project, []string{"1", "2"})
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
				ID:        fmt.Sprintf("test%d", i),
				Project:   project,
				Title:     fmt.Sprintf("Test %d", i),
				CreatedAt: time.Now().Add(-time.Duration(i) * time.Hour),
			}
			// Pin the first MaxPinnedScratches
			if i < store.MaxPinnedScratches-1 { // Leave one slot
				scratch.IsPinned = true
				scratch.PinnedAt = time.Now()
			}
			err = st.AddScratch(scratch)
			require.NoError(t, err)
		}

		// Try to pin 2 more (should fail as only 1 slot available)
		ids := []string{
			fmt.Sprintf("%d", store.MaxPinnedScratches),
			fmt.Sprintf("%d", store.MaxPinnedScratches+1),
		}
		_, err = PinMultiple(st, false, false, project, ids)
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
			ID:        "test1",
			Project:   project,
			Title:     "Test 1",
			CreatedAt: time.Now(),
		}
		err = st.AddScratch(scratch)
		require.NoError(t, err)

		// Pin with duplicate IDs
		pinnedTitles, err := PinMultiple(st, false, false, project, []string{"1", "1", "1"})
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
		unpinnedTitles, err := UnpinMultiple(st, false, false, project, []string{"p1", "p3"})
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
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Try to unpin both
		unpinnedTitles, err := UnpinMultiple(st, false, false, project, []string{"1", "2"})
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

		// Create scratches
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
			},
		}

		for _, s := range scratches {
			err = st.AddScratch(s)
			require.NoError(t, err)
		}

		// Unpin using mixed indices (regular and pinned)
		unpinnedTitles, err := UnpinMultiple(st, false, false, project, []string{"test1", "p2"})
		assert.NoError(t, err)
		assert.Equal(t, 2, len(unpinnedTitles))
		assert.Contains(t, unpinnedTitles, "Test 1")
		assert.Contains(t, unpinnedTitles, "Test 2")
	})
}
