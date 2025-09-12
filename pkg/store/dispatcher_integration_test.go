package store

import (
	"path/filepath"
	"testing"
)

func TestDispatcherIDResolution(t *testing.T) {
	tempDir := t.TempDir()
	dispatcher := NewDispatcher()

	// Create test stores
	projectDir := filepath.Join(tempDir, "myproject")
	globalDir := filepath.Join(tempDir, "global")

	projectStore, err := NewStore(projectDir)
	if err != nil {
		t.Fatalf("Failed to create project store: %v", err)
	}
	dispatcher.stores["myproject"] = projectStore

	globalStore, err := NewStore(globalDir)
	if err != nil {
		t.Fatalf("Failed to create global store: %v", err)
	}
	dispatcher.stores["global"] = globalStore

	// Add content to both stores with same UserID
	projectPad, err := projectStore.Create("Project content", "Project Title")
	if err != nil {
		t.Fatalf("Failed to create project pad: %v", err)
	}

	globalPad, err := globalStore.Create("Global content", "Global Title")
	if err != nil {
		t.Fatalf("Failed to create global pad: %v", err)
	}

	// Both should have UserID 1
	if projectPad.UserID != 1 || globalPad.UserID != 1 {
		t.Errorf("Expected both pads to have UserID 1, got %d and %d", projectPad.UserID, globalPad.UserID)
	}

	// Test explicit ID parsing
	t.Run("ExplicitIDParsing", func(t *testing.T) {
		// Test explicit project ID
		scopedID, err := dispatcher.ParseID("myproject-1", "myproject")
		if err != nil {
			t.Fatalf("Failed to parse explicit project ID: %v", err)
		}
		if scopedID.Scope != "myproject" || scopedID.UserID != 1 {
			t.Errorf("Expected myproject-1, got %s-%d", scopedID.Scope, scopedID.UserID)
		}

		// Test explicit global ID
		scopedID, err = dispatcher.ParseID("global-1", "myproject")
		if err != nil {
			t.Fatalf("Failed to parse explicit global ID: %v", err)
		}
		if scopedID.Scope != "global" || scopedID.UserID != 1 {
			t.Errorf("Expected global-1, got %s-%d", scopedID.Scope, scopedID.UserID)
		}
	})

	// Test implicit ID resolution - project scope precedence
	t.Run("ImplicitIDResolution", func(t *testing.T) {
		// When in project scope, implicit ID should resolve to project first
		scopedID, err := dispatcher.ParseID("1", "myproject")
		if err != nil {
			t.Fatalf("Failed to parse implicit ID in project scope: %v", err)
		}
		if scopedID.Scope != "myproject" {
			t.Errorf("Expected implicit ID to resolve to myproject, got %s", scopedID.Scope)
		}

		// When in global scope, implicit ID should find global
		scopedID, err = dispatcher.ParseID("1", "global")
		if err != nil {
			t.Fatalf("Failed to parse implicit ID in global scope: %v", err)
		}
		if scopedID.Scope != "global" {
			t.Errorf("Expected implicit ID to resolve to global, got %s", scopedID.Scope)
		}
	})

	// Test GetPad with different ID formats
	t.Run("GetPadWithIDFormats", func(t *testing.T) {
		// Test explicit project ID
		pad, content, scope, err := dispatcher.GetPad("myproject-1", "myproject")
		if err != nil {
			t.Fatalf("Failed to get pad with explicit project ID: %v", err)
		}
		if scope != "myproject" || pad.UserID != 1 || content != "Project content" {
			t.Errorf("Got wrong pad: scope=%s, userID=%d, content=%s", scope, pad.UserID, content)
		}

		// Test explicit global ID
		pad, content, scope, err = dispatcher.GetPad("global-1", "myproject")
		if err != nil {
			t.Fatalf("Failed to get pad with explicit global ID: %v", err)
		}
		if scope != "global" || pad.UserID != 1 || content != "Global content" {
			t.Errorf("Got wrong pad: scope=%s, userID=%d, content=%s", scope, pad.UserID, content)
		}

		// Test implicit ID (should resolve to project due to precedence)
		pad, content, scope, err = dispatcher.GetPad("1", "myproject")
		if err != nil {
			t.Fatalf("Failed to get pad with implicit ID: %v", err)
		}
		if scope != "myproject" || content != "Project content" || pad.Title != "Project Title" {
			t.Errorf("Implicit ID resolved incorrectly: scope=%s, content=%s, title=%s", scope, content, pad.Title)
		}
	})

	// Test CreatePad
	t.Run("CreatePad", func(t *testing.T) {
		pad, err := dispatcher.CreatePad("New content", "New Title", "myproject")
		if err != nil {
			t.Fatalf("Failed to create pad via dispatcher: %v", err)
		}

		// Should get UserID 2 (after the first pad)
		if pad.UserID != 2 {
			t.Errorf("Expected UserID 2, got %d", pad.UserID)
		}

		// Verify it's actually stored
		retrievedPad, content, scope, err := dispatcher.GetPad("myproject-2", "myproject")
		if err != nil {
			t.Fatalf("Failed to retrieve newly created pad: %v", err)
		}
		if scope != "myproject" || content != "New content" || retrievedPad.Title != "New Title" {
			t.Errorf("Retrieved pad doesn't match created pad")
		}
	})
}

func TestDispatcherErrorHandling(t *testing.T) {
	dispatcher := NewDispatcher()

	// Test invalid ID formats
	t.Run("InvalidIDFormats", func(t *testing.T) {
		_, err := dispatcher.ParseID("invalid", "myproject")
		if err == nil {
			t.Error("Expected error for non-numeric implicit ID")
		}

		_, err = dispatcher.ParseID("scope-invalid", "myproject")
		if err == nil {
			t.Error("Expected error for non-numeric explicit ID")
		}

		_, err = dispatcher.ParseID("scope-0", "myproject")
		if err == nil {
			t.Error("Expected error for zero ID")
		}

		_, err = dispatcher.ParseID("scope-", "myproject")
		if err == nil {
			t.Error("Expected error for empty ID part")
		}
	})

	// Test non-existent pads
	t.Run("NonExistentPads", func(t *testing.T) {
		_, _, _, err := dispatcher.GetPad("nonexistent-1", "myproject")
		if err == nil {
			t.Error("Expected error for non-existent scope")
		}

		// Create empty store but try to get non-existent pad
		tempDir := t.TempDir()
		store, _ := NewStore(tempDir)
		dispatcher.stores["empty"] = store

		_, _, _, err = dispatcher.GetPad("empty-999", "myproject")
		if err == nil {
			t.Error("Expected error for non-existent pad ID")
		}

		_, err = dispatcher.ParseID("999", "empty")
		if err == nil {
			t.Error("Expected error when implicit ID not found in any scope")
		}
	})
}

func TestFormatExplicitID(t *testing.T) {
	tests := []struct {
		scope  string
		userID int
		want   string
	}{
		{"global", 1, "global-1"},
		{"myproject", 5, "myproject-5"},
		{"test-scope", 123, "test-scope-123"},
	}

	for _, tt := range tests {
		got := FormatExplicitID(tt.scope, tt.userID)
		if got != tt.want {
			t.Errorf("FormatExplicitID(%q, %d) = %q, want %q", tt.scope, tt.userID, got, tt.want)
		}
	}
}
