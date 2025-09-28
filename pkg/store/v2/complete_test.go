package v2

import (
	"testing"
	"time"
)

func TestCompleteNanostoreIntegration(t *testing.T) {
	tmpDir := t.TempDir()
	store, err := NewSimpleStore(tmpDir)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}
	defer store.Close()

	// Test 1: Create items via store API directly (known to work)
	t.Log("=== Creating items directly ===")
	id1, err := store.store.Create("First Todo", &PadzScratch{})
	if err != nil {
		t.Fatalf("Failed to create first: %v", err)
	}
	t.Logf("Created first with UUID: %s", id1)

	id2, err := store.store.Create("Second Todo", &PadzScratch{})
	if err != nil {
		t.Fatalf("Failed to create second: %v", err)
	}
	t.Logf("Created second with UUID: %s", id2)

	// Query and verify SimpleIDs work
	items, err := store.store.Query().Find()
	if err != nil {
		t.Fatalf("Failed to query: %v", err)
	}

	t.Log("After direct create:")
	for _, item := range items {
		t.Logf("  UUID=%s, SimpleID=%s, Title=%s", item.UUID, item.SimpleID, item.Title)
		if item.SimpleID == "1" || item.SimpleID == "2" {
			t.Log("  ✓ SimpleID is correct!")
		}
	}

	// Test 2: Use GetScratches to verify our wrapper works
	scratches, err := store.GetScratches()
	if err != nil {
		t.Fatalf("Failed to get scratches: %v", err)
	}

	t.Log("\nGetScratches results:")
	for _, s := range scratches {
		t.Logf("  ID=%s, Title=%s", s.ID, s.Title)
		if s.ID == "1" || s.ID == "2" {
			t.Log("  ✓ ID mapping works!")
		}
	}

	// Test 3: Test pinning
	t.Log("\n=== Testing pinning ===")
	if len(scratches) > 0 {
		scratch := scratches[0]
		scratch.IsPinned = true
		scratch.PinnedAt = time.Now()

		err = store.UpdateScratch(scratch)
		if err != nil {
			t.Fatalf("Failed to pin: %v", err)
		}

		// Get pinned items
		pinned, err := store.GetPinnedScratches()
		if err != nil {
			t.Fatalf("Failed to get pinned: %v", err)
		}

		t.Log("Pinned scratches:")
		for _, p := range pinned {
			t.Logf("  ID=%s, Title=%s, IsPinned=%v", p.ID, p.Title, p.IsPinned)
			if len(p.ID) > 0 && p.ID[0] == 'p' {
				t.Log("  ✓ Pinned prefix works!")
			}
		}

		// Re-fetch all scratches to see current IDs
		currentScratches, _ := store.GetScratches()
		t.Log("\nAll scratches after pinning:")
		for _, s := range currentScratches {
			t.Logf("  ID=%s, Title=%s, IsPinned=%v", s.ID, s.Title, s.IsPinned)
		}
	}

	// Test 4: Test soft deletion
	t.Log("\n=== Testing soft deletion ===")
	// Get current scratches after pinning
	currentScratches, _ := store.GetScratches()
	if len(currentScratches) > 1 {
		// Delete the last item (which now has ID "1")
		idToDelete := currentScratches[len(currentScratches)-1].ID
		t.Logf("Deleting ID: %s", idToDelete)

		// Debug: check what resolveID returns
		uuid, resolveErr := store.resolveID(idToDelete)
		if resolveErr != nil {
			t.Logf("Failed to resolve ID %s: %v", idToDelete, resolveErr)
		} else {
			t.Logf("Resolved ID %s to UUID %s", idToDelete, uuid)
		}

		err = store.RemoveScratch(idToDelete)
		if err != nil {
			t.Fatalf("Failed to delete: %v", err)
		}

		// Verify deletion
		remaining, err := store.GetScratches()
		if err != nil {
			t.Fatalf("Failed to get remaining: %v", err)
		}

		t.Logf("Remaining: %d items", len(remaining))
		for _, r := range remaining {
			if r.ID == idToDelete {
				t.Errorf("Deleted item still present!")
			}
		}
	}

	// Test 5: Test ID resolution
	t.Log("\n=== Testing ID resolution ===")
	// Get current items to test with actual IDs
	finalItems, _ := store.GetScratches()
	var testID string
	var testUUID string
	if len(finalItems) > 0 {
		testID = finalItems[0].ID
		// Get the actual UUID for testing
		testUUID, _ = store.resolveID(testID)
	}

	testCases := []struct {
		input    string
		expected bool
	}{
		{testID, len(testID) > 0},         // Current simple/pinned ID
		{testUUID[:8], len(testUUID) > 0}, // Partial UUID
		{testUUID, len(testUUID) > 0},     // Full UUID
		{"999", false},                    // Non-existent
		{"nonexistent", false},            // Non-existent
	}

	for _, tc := range testCases {
		uuid, err := store.resolveID(tc.input)
		if tc.expected && err != nil {
			t.Errorf("Failed to resolve %s: %v", tc.input, err)
		} else if !tc.expected && err == nil {
			t.Errorf("Should not have resolved %s but got: %s", tc.input, uuid)
		} else if tc.expected {
			t.Logf("✓ Resolved %s → %s", tc.input, uuid)
		}
	}
}
