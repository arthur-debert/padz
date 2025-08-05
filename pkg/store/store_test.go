package store

import (
	"fmt"
	"reflect"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/testutil"
)

func TestNewStore(t *testing.T) {
	tests := []struct {
		name          string
		setupFiles    map[string]string
		expectError   bool
		expectScratch int
	}{
		{
			name:          "empty store initialization",
			setupFiles:    nil,
			expectError:   false,
			expectScratch: 0,
		},
		{
			name: "load existing metadata",
			setupFiles: map[string]string{
				"metadata.json": `[{"id":"test1","project":"testproj","title":"Test Scratch","created_at":"2023-01-01T00:00:00Z"}]`,
			},
			expectError:   false,
			expectScratch: 1,
		},
		{
			name: "invalid json metadata",
			setupFiles: map[string]string{
				"metadata.json": `invalid json`,
			},
			expectError:   true,
			expectScratch: 0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cfg, cleanup := testutil.SetupTestEnvironment(t)
			defer cleanup()

			// Setup files in memory filesystem
			if tt.setupFiles != nil {
				for filename, content := range tt.setupFiles {
					path := cfg.FileSystem.Join("/test/data/scratch", filename)
					err := cfg.FileSystem.WriteFile(path, []byte(content), 0644)
					if err != nil {
						t.Fatalf("Failed to write test file: %v", err)
					}
				}
			}

			store, err := NewStoreWithConfig(cfg)
			if tt.expectError {
				if err == nil {
					t.Errorf("expected error but got none")
				}
				return
			}
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			scratches := store.GetScratches()
			if len(scratches) != tt.expectScratch {
				t.Errorf("expected %d scratches, got %d", tt.expectScratch, len(scratches))
			}
		})
	}
}

func TestStore_GetScratches(t *testing.T) {
	testScratch := Scratch{
		ID:        "test123",
		Project:   "testproject",
		Title:     "Test Title",
		CreatedAt: time.Date(2023, 1, 1, 0, 0, 0, 0, time.UTC),
	}

	store := &Store{
		scratches: []Scratch{testScratch},
	}

	scratches := store.GetScratches()
	if len(scratches) != 1 {
		t.Errorf("expected 1 scratch, got %d", len(scratches))
	}
	if !reflect.DeepEqual(scratches[0], testScratch) {
		t.Errorf("expected %+v, got %+v", testScratch, scratches[0])
	}
}

func TestStore_AddScratch(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	store, err := NewStoreWithConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	testScratch := Scratch{
		ID:        "test123",
		Project:   "testproject",
		Title:     "Test Title",
		CreatedAt: time.Now(),
	}

	err = store.AddScratch(testScratch)
	if err != nil {
		t.Errorf("unexpected error adding scratch: %v", err)
	}

	scratches := store.GetScratches()
	if len(scratches) != 1 {
		t.Errorf("expected 1 scratch, got %d", len(scratches))
	}
	if scratches[0].ID != testScratch.ID {
		t.Errorf("expected ID %s, got %s", testScratch.ID, scratches[0].ID)
	}
}

func TestStore_RemoveScratch(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	store, err := NewStoreWithConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	tests := []struct {
		name             string
		initialScratches []Scratch
		removeID         string
		expectedCount    int
		expectedRemoved  bool
	}{
		{
			name: "remove existing scratch",
			initialScratches: []Scratch{
				{ID: "test1", Project: "proj1", Title: "Title 1", CreatedAt: time.Now()},
				{ID: "test2", Project: "proj2", Title: "Title 2", CreatedAt: time.Now()},
			},
			removeID:        "test1",
			expectedCount:   1,
			expectedRemoved: true,
		},
		{
			name: "remove non-existent scratch",
			initialScratches: []Scratch{
				{ID: "test1", Project: "proj1", Title: "Title 1", CreatedAt: time.Now()},
			},
			removeID:        "nonexistent",
			expectedCount:   1,
			expectedRemoved: false,
		},
		{
			name:             "remove from empty store",
			initialScratches: []Scratch{},
			removeID:         "test1",
			expectedCount:    0,
			expectedRemoved:  false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			store.scratches = make([]Scratch, len(tt.initialScratches))
			copy(store.scratches, tt.initialScratches)

			err := store.RemoveScratch(tt.removeID)
			if err != nil {
				t.Errorf("unexpected error removing scratch: %v", err)
			}

			scratches := store.GetScratches()
			if len(scratches) != tt.expectedCount {
				t.Errorf("expected %d scratches, got %d", tt.expectedCount, len(scratches))
			}

			found := false
			for _, scratch := range scratches {
				if scratch.ID == tt.removeID {
					found = true
					break
				}
			}
			if found && tt.expectedRemoved {
				t.Errorf("expected scratch %s to be removed but it's still there", tt.removeID)
			}
		})
	}
}

func TestStore_UpdateScratch(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	store, err := NewStoreWithConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	originalScratch := Scratch{
		ID:        "test1",
		Project:   "original",
		Title:     "Original Title",
		CreatedAt: time.Date(2023, 1, 1, 0, 0, 0, 0, time.UTC),
	}

	store.scratches = []Scratch{originalScratch}

	updatedScratch := Scratch{
		ID:        "test1",
		Project:   "updated",
		Title:     "Updated Title",
		CreatedAt: time.Date(2023, 2, 1, 0, 0, 0, 0, time.UTC),
	}

	err = store.UpdateScratch(updatedScratch)
	if err != nil {
		t.Errorf("unexpected error updating scratch: %v", err)
	}

	scratches := store.GetScratches()
	if len(scratches) != 1 {
		t.Errorf("expected 1 scratch, got %d", len(scratches))
	}
	if scratches[0].Title != "Updated Title" {
		t.Errorf("expected title 'Updated Title', got '%s'", scratches[0].Title)
	}
	if scratches[0].Project != "updated" {
		t.Errorf("expected project 'updated', got '%s'", scratches[0].Project)
	}
}

func TestStore_SaveScratches(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	store, err := NewStoreWithConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	testScratches := []Scratch{
		{ID: "test1", Project: "proj1", Title: "Title 1", CreatedAt: time.Now()},
		{ID: "test2", Project: "proj2", Title: "Title 2", CreatedAt: time.Now()},
	}

	err = store.SaveScratches(testScratches)
	if err != nil {
		t.Errorf("unexpected error saving scratches: %v", err)
	}

	scratches := store.GetScratches()
	if len(scratches) != 2 {
		t.Errorf("expected 2 scratches, got %d", len(scratches))
	}
}

func TestGetScratchPath(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	path, err := GetScratchPathWithConfig(cfg)
	if err != nil {
		t.Errorf("unexpected error getting scratch path: %v", err)
	}

	expectedPath := "/test/data/scratch"
	if path != expectedPath {
		t.Errorf("expected path %s, got %s", expectedPath, path)
	}

	// Verify directory was created in memory filesystem
	if _, err := cfg.FileSystem.Stat(path); err != nil {
		t.Errorf("expected directory to be created at %s", path)
	}
}

func TestGetScratchFilePath(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	tests := []struct {
		name       string
		id         string
		expectPath string
	}{
		{
			name:       "simple id",
			id:         "test123",
			expectPath: "/test/data/scratch/test123",
		},
		{
			name:       "hash id",
			id:         "abc123def456",
			expectPath: "/test/data/scratch/abc123def456",
		},
		{
			name:       "empty id",
			id:         "",
			expectPath: "/test/data/scratch",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			path, err := GetScratchFilePathWithConfig(tt.id, cfg)
			if err != nil {
				t.Errorf("unexpected error getting scratch file path: %v", err)
			}

			if path != tt.expectPath {
				t.Errorf("expected path %s, got %s", tt.expectPath, path)
			}
		})
	}
}

func TestStore_ConcurrentAccess(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	store, err := NewStoreWithConfig(cfg)
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	const numGoroutines = 10
	const numOperations = 100

	done := make(chan bool, numGoroutines)
	errors := make(chan error, numGoroutines*numOperations)

	for i := 0; i < numGoroutines; i++ {
		go func(id int) {
			for j := 0; j < numOperations; j++ {
				scratch := Scratch{
					ID:        fmt.Sprintf("test_%d_%d", id, j),
					Project:   fmt.Sprintf("proj_%d", id),
					Title:     fmt.Sprintf("Title %d %d", id, j),
					CreatedAt: time.Now(),
				}
				if err := store.AddScratch(scratch); err != nil {
					errors <- fmt.Errorf("failed to add scratch: %v", err)
					return
				}
			}
			done <- true
		}(i)
	}

	for i := 0; i < numGoroutines; i++ {
		<-done
	}
	close(errors)

	// Check if any errors occurred
	for err := range errors {
		if err != nil {
			t.Fatal(err)
		}
	}

	scratches := store.GetScratches()
	expected := numGoroutines * numOperations
	if len(scratches) != expected {
		t.Errorf("expected %d scratches, got %d", expected, len(scratches))
	}
}

func TestMemoryFilesystemIntegration(t *testing.T) {
	cfg, cleanup := testutil.SetupTestEnvironment(t)
	defer cleanup()

	// Verify we're using memory filesystem
	memFS := testutil.GetMemoryFS(cfg)
	if memFS == nil {
		t.Fatal("Expected memory filesystem in test configuration")
	}

	// Create a store and add scratches
	store, err := NewStoreWithConfig(cfg)
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}

	// Add some scratches
	for i := 0; i < 3; i++ {
		scratch := Scratch{
			ID:        fmt.Sprintf("test%d", i),
			Project:   "testproject",
			Title:     fmt.Sprintf("Test %d", i),
			CreatedAt: time.Now(),
		}
		if err := store.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
	}

	// Verify files are in memory filesystem only
	files := memFS.GetAllFiles()

	// Should have metadata file
	metadataPath := "/test/data/scratch/metadata.json"
	if _, exists := files[metadataPath]; !exists {
		t.Error("Expected metadata file in memory filesystem")
	}

	// Verify content
	data, err := cfg.FileSystem.ReadFile(metadataPath)
	if err != nil {
		t.Fatalf("Failed to read metadata: %v", err)
	}

	if len(data) == 0 {
		t.Error("Metadata file is empty")
	}
}
