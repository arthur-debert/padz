package store

import (
	"fmt"
	"os"
	"path/filepath"
	"reflect"
	"testing"
	"time"

	"github.com/adrg/xdg"
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
			tmpDir := setupTempDir(t, tt.setupFiles)
			defer os.RemoveAll(tmpDir)

			// Mock the XDG data directory
			oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
			os.Setenv("XDG_DATA_HOME", tmpDir)
			xdg.Reload()
			defer func() {
				if oldXDGDataHome == "" {
					os.Unsetenv("XDG_DATA_HOME")
				} else {
					os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
				}
				xdg.Reload()
			}()

			store, err := NewStore()
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
	tmpDir := setupTempDir(t, nil)
	defer os.RemoveAll(tmpDir)

	oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
	os.Setenv("XDG_DATA_HOME", tmpDir)
	xdg.Reload()
	defer func() {
		if oldXDGDataHome == "" {
			os.Unsetenv("XDG_DATA_HOME")
		} else {
			os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
		}
		xdg.Reload()
	}()

	store, err := NewStore()
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
	tmpDir := setupTempDir(t, nil)
	defer os.RemoveAll(tmpDir)

	oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
	os.Setenv("XDG_DATA_HOME", tmpDir)
	xdg.Reload()
	defer func() {
		if oldXDGDataHome == "" {
			os.Unsetenv("XDG_DATA_HOME")
		} else {
			os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
		}
		xdg.Reload()
	}()

	store, err := NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	tests := []struct {
		name            string
		initialScratches []Scratch
		removeID        string
		expectedCount   int
		expectedRemoved bool
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
			name:            "remove from empty store",
			initialScratches: []Scratch{},
			removeID:        "test1",
			expectedCount:   0,
			expectedRemoved: false,
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
	tmpDir := setupTempDir(t, nil)
	defer os.RemoveAll(tmpDir)

	oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
	os.Setenv("XDG_DATA_HOME", tmpDir)
	xdg.Reload()
	defer func() {
		if oldXDGDataHome == "" {
			os.Unsetenv("XDG_DATA_HOME")
		} else {
			os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
		}
		xdg.Reload()
	}()

	store, err := NewStore()
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
	tmpDir := setupTempDir(t, nil)
	defer os.RemoveAll(tmpDir)

	oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
	os.Setenv("XDG_DATA_HOME", tmpDir)
	xdg.Reload()
	defer func() {
		if oldXDGDataHome == "" {
			os.Unsetenv("XDG_DATA_HOME")
		} else {
			os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
		}
		xdg.Reload()
	}()

	store, err := NewStore()
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
	tmpDir := t.TempDir()
	oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
	os.Setenv("XDG_DATA_HOME", tmpDir)
	xdg.Reload()
	defer func() {
		if oldXDGDataHome == "" {
			os.Unsetenv("XDG_DATA_HOME")
		} else {
			os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
		}
		xdg.Reload()
	}()

	path, err := GetScratchPath()
	if err != nil {
		t.Errorf("unexpected error getting scratch path: %v", err)
	}

	expectedPath := filepath.Join(tmpDir, "scratch")
	if path != expectedPath {
		t.Errorf("expected path %s, got %s", expectedPath, path)
	}

	if _, err := os.Stat(path); os.IsNotExist(err) {
		t.Errorf("expected directory to be created at %s", path)
	}
}

func TestGetScratchFilePath(t *testing.T) {
	tmpDir := t.TempDir()
	oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
	os.Setenv("XDG_DATA_HOME", tmpDir)
	xdg.Reload()
	defer func() {
		if oldXDGDataHome == "" {
			os.Unsetenv("XDG_DATA_HOME")
		} else {
			os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
		}
		xdg.Reload()
	}()

	tests := []struct {
		name       string
		id         string
		expectPath string
	}{
		{
			name:       "simple id",
			id:         "test123",
			expectPath: "test123",
		},
		{
			name:       "hash id",
			id:         "abc123def456",
			expectPath: "abc123def456",
		},
		{
			name:       "empty id",
			id:         "",
			expectPath: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			path, err := GetScratchFilePath(tt.id)
			if err != nil {
				t.Errorf("unexpected error getting scratch file path: %v", err)
			}

			expectedPath := filepath.Join(tmpDir, "scratch", tt.expectPath)
			if path != expectedPath {
				t.Errorf("expected path %s, got %s", expectedPath, path)
			}
		})
	}
}

func TestStore_ConcurrentAccess(t *testing.T) {
	tmpDir := setupTempDir(t, nil)
	defer os.RemoveAll(tmpDir)

	oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
	os.Setenv("XDG_DATA_HOME", tmpDir)
	xdg.Reload()
	defer func() {
		if oldXDGDataHome == "" {
			os.Unsetenv("XDG_DATA_HOME")
		} else {
			os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
		}
		xdg.Reload()
	}()

	store, err := NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	const numGoroutines = 10
	const numOperations = 100

	done := make(chan bool, numGoroutines)

	for i := 0; i < numGoroutines; i++ {
		go func(id int) {
			for j := 0; j < numOperations; j++ {
				scratch := Scratch{
					ID:        fmt.Sprintf("test_%d_%d", id, j),
					Project:   fmt.Sprintf("proj_%d", id),
					Title:     fmt.Sprintf("Title %d %d", id, j),
					CreatedAt: time.Now(),
				}
				store.AddScratch(scratch)
			}
			done <- true
		}(i)
	}

	for i := 0; i < numGoroutines; i++ {
		<-done
	}

	scratches := store.GetScratches()
	expected := numGoroutines * numOperations
	if len(scratches) != expected {
		t.Errorf("expected %d scratches, got %d", expected, len(scratches))
	}
}

func setupTempDir(t *testing.T, files map[string]string) string {
	tmpDir := t.TempDir()
	scratchDir := filepath.Join(tmpDir, "scratch")
	if err := os.MkdirAll(scratchDir, 0755); err != nil {
		t.Fatalf("failed to create scratch directory: %v", err)
	}

	for filename, content := range files {
		path := filepath.Join(scratchDir, filename)
		if err := os.WriteFile(path, []byte(content), 0644); err != nil {
			t.Fatalf("failed to write test file %s: %v", filename, err)
		}
	}

	return tmpDir
}