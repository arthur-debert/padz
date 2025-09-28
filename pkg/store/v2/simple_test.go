package v2

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestSimpleStore(t *testing.T) {
	tmpDir := t.TempDir()
	store, err := NewSimpleStore(tmpDir)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}
	defer store.Close()

	// Test 1: Add a scratch
	t.Run("AddScratch", func(t *testing.T) {
		scratch := Scratch{
			ID:        "test-1",
			Title:     "Test Scratch",
			Project:   "test-project",
			CreatedAt: time.Now(),
			UpdatedAt: time.Now(),
		}

		id, err := store.AddScratch(scratch)
		if err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}

		t.Logf("Added scratch with ID: %s", id)

		// Verify it was added
		scratches, err := store.GetScratches()
		if err != nil {
			t.Fatalf("Failed to get scratches: %v", err)
		}

		if len(scratches) != 1 {
			t.Errorf("Expected 1 scratch, got %d", len(scratches))
		}

		if len(scratches) > 0 {
			t.Logf("Scratch: ID=%s, Title=%s, SimpleID=%s",
				scratches[0].ID, scratches[0].Title, scratches[0].ID)
		}
	})

	// Test 2: Add multiple and check IDs
	t.Run("MultipleScratches", func(t *testing.T) {
		for i := 2; i <= 4; i++ {
			scratch := Scratch{
				ID:        fmt.Sprintf("test-%d", i),
				Title:     fmt.Sprintf("Scratch %d", i),
				Project:   "test-project",
				CreatedAt: time.Now().Add(-time.Duration(i) * time.Hour),
				UpdatedAt: time.Now(),
			}
			_, err := store.AddScratch(scratch)
			if err != nil {
				t.Fatalf("Failed to add scratch %d: %v", i, err)
			}
		}

		scratches, err := store.GetScratches()
		if err != nil {
			t.Fatalf("Failed to get scratches: %v", err)
		}

		t.Logf("Total scratches: %d", len(scratches))
		for i, s := range scratches {
			t.Logf("  [%d] ID=%s, Title=%s", i, s.ID, s.Title)
		}
	})

	// Test 3: Test pinning
	t.Run("Pinning", func(t *testing.T) {
		scratches, err := store.GetScratches()
		if err != nil {
			t.Fatalf("Failed to get scratches: %v", err)
		}

		if len(scratches) < 2 {
			t.Skip("Not enough scratches for pinning test")
		}

		// Pin first scratch
		scratch := scratches[0]
		scratch.IsPinned = true
		scratch.PinnedAt = time.Now()

		err = store.UpdateScratch(scratch)
		if err != nil {
			t.Fatalf("Failed to update scratch: %v", err)
		}

		// Get pinned scratches
		pinned, err := store.GetPinnedScratches()
		if err != nil {
			t.Fatalf("Failed to get pinned scratches: %v", err)
		}

		t.Logf("Pinned scratches: %d", len(pinned))
		for i, p := range pinned {
			t.Logf("  [%d] ID=%s, Title=%s, IsPinned=%v",
				i, p.ID, p.Title, p.IsPinned)
		}

		if len(pinned) != 1 {
			t.Errorf("Expected 1 pinned scratch, got %d", len(pinned))
		}
	})

	// Test 4: Test soft deletion
	t.Run("SoftDelete", func(t *testing.T) {
		scratches, err := store.GetScratches()
		if err != nil {
			t.Fatalf("Failed to get scratches: %v", err)
		}

		initialCount := len(scratches)
		t.Logf("Initial scratch count: %d", initialCount)

		if initialCount > 0 {
			// Delete first scratch
			err = store.RemoveScratch(scratches[0].ID)
			if err != nil {
				t.Fatalf("Failed to remove scratch: %v", err)
			}

			// Check remaining
			remaining, err := store.GetScratches()
			if err != nil {
				t.Fatalf("Failed to get remaining scratches: %v", err)
			}

			t.Logf("Remaining scratch count: %d", len(remaining))
			if len(remaining) != initialCount-1 {
				t.Errorf("Expected %d scratches after deletion, got %d",
					initialCount-1, len(remaining))
			}
		}
	})

	// Verify store file exists
	storePath := filepath.Join(tmpDir, storeFileName)
	if _, err := os.Stat(storePath); os.IsNotExist(err) {
		t.Errorf("Store file not created at %s", storePath)
	}
}
