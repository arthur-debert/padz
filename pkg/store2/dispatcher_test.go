package store2

import (
	"path/filepath"
	"testing"
)

func TestDispatcherStoreManagement(t *testing.T) {
	tempDir := t.TempDir()
	dispatcher := NewDispatcher()

	// Create store directories
	scope1Dir := filepath.Join(tempDir, "scope1")
	scope2Dir := filepath.Join(tempDir, "scope2")

	// Manually create stores for testing
	store1, err := NewStore(scope1Dir)
	if err != nil {
		t.Fatalf("Failed to create store1: %v", err)
	}
	dispatcher.stores["scope1"] = store1

	store2, err := NewStore(scope2Dir)
	if err != nil {
		t.Fatalf("Failed to create store2: %v", err)
	}
	dispatcher.stores["scope2"] = store2

	// Test getting cached stores
	retrievedStore1 := dispatcher.stores["scope1"]
	if retrievedStore1 != store1 {
		t.Error("Store1 not properly cached")
	}

	retrievedStore2 := dispatcher.stores["scope2"]
	if retrievedStore2 != store2 {
		t.Error("Store2 not properly cached")
	}

	// Verify stores are different
	if store1 == store2 {
		t.Error("Expected different store instances for different scopes")
	}

	// Add content to verify stores work
	pad1, err := store1.Create("Content 1", "Title 1")
	if err != nil {
		t.Fatalf("Failed to create pad in store1: %v", err)
	}

	pad2, err := store2.Create("Content 2", "Title 2")
	if err != nil {
		t.Fatalf("Failed to create pad in store2: %v", err)
	}

	// Verify pads are in correct stores
	if pad1.UserID != 1 {
		t.Errorf("Expected UserID 1 for pad1, got %d", pad1.UserID)
	}

	if pad2.UserID != 1 {
		t.Errorf("Expected UserID 1 for pad2, got %d", pad2.UserID)
	}

	// Verify stores are independent
	pads1, err := store1.List()
	if err != nil {
		t.Fatalf("Failed to list pads from store1: %v", err)
	}

	pads2, err := store2.List()
	if err != nil {
		t.Fatalf("Failed to list pads from store2: %v", err)
	}

	if len(pads1) != 1 || len(pads2) != 1 {
		t.Errorf("Expected 1 pad in each store, got %d and %d", len(pads1), len(pads2))
	}

	if pads1[0].Title != "Title 1" {
		t.Errorf("Expected 'Title 1' in store1, got %s", pads1[0].Title)
	}

	if pads2[0].Title != "Title 2" {
		t.Errorf("Expected 'Title 2' in store2, got %s", pads2[0].Title)
	}
}
