package v2

import (
	"testing"
	"time"
)

func TestDebugSimpleStore(t *testing.T) {
	tmpDir := t.TempDir()
	store, err := NewSimpleStore(tmpDir)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}
	defer store.Close()

	// Test 1: Create items directly via nanostore API
	t.Log("=== Creating items directly via API ===")

	id1, err := store.store.Create("Direct Item 1", &PadzScratch{})
	if err != nil {
		t.Fatalf("Failed to create direct item 1: %v", err)
	}
	t.Logf("Created direct item 1 with ID: %s", id1)

	id2, err := store.store.Create("Direct Item 2", &PadzScratch{})
	if err != nil {
		t.Fatalf("Failed to create direct item 2: %v", err)
	}
	t.Logf("Created direct item 2 with ID: %s", id2)

	// Query directly
	directResults, err := store.store.Query().Activity("active").Find()
	if err != nil {
		t.Fatalf("Failed to query direct results: %v", err)
	}

	t.Logf("Direct query results: %d items", len(directResults))
	for i, r := range directResults {
		t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s",
			i, r.UUID, r.SimpleID, r.Title)
	}

	// Test 2: Create via SimpleStore wrapper
	t.Log("\n=== Creating items via SimpleStore wrapper ===")

	scratch1 := Scratch{
		ID:        "wrapper-1",
		Title:     "Wrapper Item 1",
		Project:   "test",
		CreatedAt: time.Now(),
		UpdatedAt: time.Now(),
	}
	id3, err := store.AddScratch(scratch1)
	if err != nil {
		t.Fatalf("Failed to create wrapper item 1: %v", err)
	}
	t.Logf("Created wrapper item 1 with ID: %s", id3)

	// Query all items now
	allResults, err := store.store.Query().Activity("active").Find()
	if err != nil {
		t.Fatalf("Failed to query all results: %v", err)
	}

	t.Logf("\nAll items after wrapper create: %d items", len(allResults))
	for i, r := range allResults {
		t.Logf("  [%d] UUID=%s, SimpleID=%s, Title=%s",
			i, r.UUID, r.SimpleID, r.Title)
	}

	// Test via GetScratches
	scratches, err := store.GetScratches()
	if err != nil {
		t.Fatalf("Failed to get scratches: %v", err)
	}

	t.Logf("\nGetScratches results: %d items", len(scratches))
	for i, s := range scratches {
		t.Logf("  [%d] ID=%s, Title=%s", i, s.ID, s.Title)
	}
}
