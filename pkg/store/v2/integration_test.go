package v2

import (
	"testing"
	"time"
)

func TestNanostoreIntegration(t *testing.T) {
	tmpDir := t.TempDir()
	store, err := NewSimpleStore(tmpDir)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}
	defer store.Close()

	// Test 1: Add scratches and verify IDs
	t.Log("=== Adding scratches ===")
	scratch1 := Scratch{
		ID:        "test-1",
		Title:     "First Scratch",
		Project:   "test-project",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	id1, err := store.AddScratch(scratch1)
	if err != nil {
		t.Fatalf("Failed to add scratch1: %v", err)
	}
	t.Logf("Added scratch1 with UUID: %s", id1)

	scratch2 := Scratch{
		ID:        "test-2",
		Title:     "Second Scratch",
		Project:   "test-project",
		CreatedAt: time.Now().Add(-1 * time.Hour),
		UpdatedAt: time.Now(),
	}
	id2, err := store.AddScratch(scratch2)
	if err != nil {
		t.Fatalf("Failed to add scratch2: %v", err)
	}
	t.Logf("Added scratch2 with UUID: %s", id2)

	// Debug: Let's check the raw query results
	rawResults, err := store.store.Query().Activity("active").Find()
	if err != nil {
		t.Fatalf("Failed to query raw results: %v", err)
	}
	t.Logf("Raw query results: %d items", len(rawResults))
	for i, r := range rawResults {
		t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s, Activity=%s",
			i, r.UUID, r.SimpleID, r.Title, r.Activity)
	}

	// Verify they were added
	scratches, err := store.GetScratches()
	if err != nil {
		t.Fatalf("Failed to get scratches: %v", err)
	}
	if len(scratches) != 2 {
		t.Errorf("Expected 2 scratches, got %d", len(scratches))
	}

	t.Log("Current scratches:")
	for i, s := range scratches {
		t.Logf("  [%d] SimpleID=%s, Title=%s, UUID=%s", i, s.ID, s.Title, s.ID)
		// Check if SimpleID is a sequential number
		if s.ID == id1 || s.ID == id2 {
			t.Errorf("SimpleID should not be UUID, got %s", s.ID)
		}
	}

	// Test 2: Pin a scratch
	t.Log("\n=== Testing pinning ===")
	if len(scratches) > 0 {
		pinnedScratch := scratches[0]
		pinnedScratch.IsPinned = true
		pinnedScratch.PinnedAt = time.Now()

		err = store.UpdateScratch(pinnedScratch)
		if err != nil {
			t.Fatalf("Failed to pin scratch: %v", err)
		}

		// Get pinned scratches
		pinned, err := store.GetPinnedScratches()
		if err != nil {
			t.Fatalf("Failed to get pinned scratches: %v", err)
		}

		if len(pinned) != 1 {
			t.Errorf("Expected 1 pinned scratch, got %d", len(pinned))
		}

		t.Log("Pinned scratches:")
		for i, p := range pinned {
			t.Logf("  [%d] SimpleID=%s, Title=%s, IsPinned=%v",
				i, p.ID, p.Title, p.IsPinned)
			// Check for 'p' prefix in pinned items
			if len(p.ID) > 0 && p.ID[0] != 'p' {
				t.Logf("Warning: Pinned item should have 'p' prefix, got %s", p.ID)
			}
		}
	}

	// Test 3: Soft deletion
	t.Log("\n=== Testing soft deletion ===")
	if len(scratches) > 0 {
		idToDelete := scratches[0].ID
		t.Logf("Deleting scratch with ID: %s", idToDelete)

		err = store.RemoveScratch(idToDelete)
		if err != nil {
			t.Fatalf("Failed to remove scratch: %v", err)
		}

		// Verify it's soft deleted
		remaining, err := store.GetScratches()
		if err != nil {
			t.Fatalf("Failed to get remaining scratches: %v", err)
		}

		if len(remaining) != len(scratches)-1 {
			t.Errorf("Expected %d scratches after deletion, got %d",
				len(scratches)-1, len(remaining))
		}

		// Verify the deleted item is not in the list
		for _, s := range remaining {
			if s.ID == idToDelete {
				t.Errorf("Deleted scratch still appears in active list: %s", idToDelete)
			}
		}

		t.Log("Remaining scratches:")
		for i, s := range remaining {
			t.Logf("  [%d] SimpleID=%s, Title=%s", i, s.ID, s.Title)
		}
	}

	// Test 4: ID resolution
	t.Log("\n=== Testing ID resolution ===")
	if len(scratches) > 0 {
		testID := scratches[0].ID
		t.Logf("Testing resolution of ID: %s", testID)

		uuid, err := store.resolveID(testID)
		if err != nil {
			t.Errorf("Failed to resolve ID %s: %v", testID, err)
		} else {
			t.Logf("Resolved %s to UUID: %s", testID, uuid)
		}

		// Test partial UUID resolution
		if len(uuid) > 8 {
			partial := uuid[:8]
			resolvedUUID, err := store.resolveID(partial)
			if err != nil {
				t.Errorf("Failed to resolve partial UUID %s: %v", partial, err)
			} else if resolvedUUID != uuid {
				t.Errorf("Partial UUID resolution mismatch: expected %s, got %s",
					uuid, resolvedUUID)
			} else {
				t.Logf("Successfully resolved partial UUID %s to %s", partial, uuid)
			}
		}
	}
}
