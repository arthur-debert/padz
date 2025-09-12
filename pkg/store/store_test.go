package store

import (
	"os"
	"path/filepath"
	"testing"
	"time"
)

func TestNewStore(t *testing.T) {
	// Create temp directory
	tempDir := t.TempDir()
	storePath := filepath.Join(tempDir, "test-store")

	// Create new store
	store, err := NewStore(storePath)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}

	// Verify directories were created
	if _, err := os.Stat(storePath); err != nil {
		t.Errorf("Store directory was not created: %v", err)
	}

	dataDir := filepath.Join(storePath, "data")
	if _, err := os.Stat(dataDir); err != nil {
		t.Errorf("Data directory was not created: %v", err)
	}

	// Verify metadata was initialized
	if store.metadata == nil {
		t.Error("Metadata was not initialized")
	}
	if store.metadata.Version != "2.0" {
		t.Errorf("Expected version 2.0, got %s", store.metadata.Version)
	}
	if store.metadata.NextID != 1 {
		t.Errorf("Expected NextID 1, got %d", store.metadata.NextID)
	}
}

func TestCreateAndGet(t *testing.T) {
	// Setup
	tempDir := t.TempDir()
	store, err := NewStore(tempDir)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}

	// Test Create
	content := "This is test content\nWith multiple lines"
	title := "Test Title"

	pad, err := store.Create(content, title)
	if err != nil {
		t.Fatalf("Failed to create pad: %v", err)
	}

	// Verify pad properties
	if pad.UserID != 1 {
		t.Errorf("Expected UserID 1, got %d", pad.UserID)
	}
	if pad.Title != title {
		t.Errorf("Expected title %q, got %q", title, pad.Title)
	}
	if pad.Size != int64(len(content)) {
		t.Errorf("Expected size %d, got %d", len(content), pad.Size)
	}

	// Test Get by user ID
	retrievedPad, retrievedContent, err := store.Get(1)
	if err != nil {
		t.Fatalf("Failed to get pad: %v", err)
	}

	if retrievedPad.ID != pad.ID {
		t.Errorf("Retrieved pad ID mismatch")
	}
	if retrievedContent != content {
		t.Errorf("Content mismatch:\nExpected: %q\nGot: %q", content, retrievedContent)
	}

	// Test GetByID
	retrievedPad2, retrievedContent2, err := store.GetByID(pad.ID)
	if err != nil {
		t.Fatalf("Failed to get pad by ID: %v", err)
	}
	if retrievedPad2.ID != pad.ID {
		t.Errorf("Retrieved pad ID mismatch")
	}
	if retrievedContent2 != content {
		t.Errorf("Content mismatch")
	}
}

func TestList(t *testing.T) {
	// Setup
	tempDir := t.TempDir()
	store, err := NewStore(tempDir)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}

	// Create multiple pads
	contents := []string{
		"First pad content",
		"Second pad content",
		"Third pad content",
	}

	for i, content := range contents {
		_, err := store.Create(content, "")
		if err != nil {
			t.Fatalf("Failed to create pad %d: %v", i+1, err)
		}
		// Small delay to ensure different timestamps
		time.Sleep(10 * time.Millisecond)
	}

	// List pads
	pads, err := store.List()
	if err != nil {
		t.Fatalf("Failed to list pads: %v", err)
	}

	// Verify count
	if len(pads) != 3 {
		t.Fatalf("Expected 3 pads, got %d", len(pads))
	}

	// Verify ordering (newest first by UserID - higher UserID = newer)
	if pads[0].UserID != 3 {
		t.Errorf("Expected newest pad to have UserID 3, got %d", pads[0].UserID)
	}

	// Verify descending UserID order (newest to oldest)
	expectedUserIDs := []int{3, 2, 1}
	for i, pad := range pads {
		if pad.UserID != expectedUserIDs[i] {
			t.Errorf("Pad %d: expected UserID %d, got %d", i, expectedUserIDs[i], pad.UserID)
		}
	}
}

func TestDeduplication(t *testing.T) {
	// Setup
	tempDir := t.TempDir()
	store, err := NewStore(tempDir)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}

	// Create first pad
	content := "Duplicate content"
	_, err = store.Create(content, "First")
	if err != nil {
		t.Fatalf("Failed to create first pad: %v", err)
	}

	// Try to create duplicate
	_, err = store.Create(content, "Second")
	if err == nil {
		t.Error("Expected error for duplicate content, got nil")
	}
}

func TestPersistence(t *testing.T) {
	// Setup
	tempDir := t.TempDir()
	storePath := filepath.Join(tempDir, "persist-store")

	// Create store and add content
	store1, err := NewStore(storePath)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}

	content := "Persistent content"
	pad, err := store1.Create(content, "Persistent")
	if err != nil {
		t.Fatalf("Failed to create pad: %v", err)
	}
	originalID := pad.ID

	// Create new store instance (simulating restart)
	storeB, err := NewStore(storePath)
	if err != nil {
		t.Fatalf("Failed to recreate store: %v", err)
	}

	// Verify content persisted
	retrievedPad, retrievedContent, err := storeB.Get(1)
	if err != nil {
		t.Fatalf("Failed to get pad from new store: %v", err)
	}

	if retrievedPad.ID != originalID {
		t.Errorf("Pad ID changed after reload")
	}
	if retrievedContent != content {
		t.Errorf("Content not persisted correctly")
	}
}
