package commands

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"

	"github.com/arthur-debert/padz/pkg/store"
)

func TestNuke(t *testing.T) {
	// Create temporary store
	tempDir := t.TempDir()
	_ = os.Setenv("XDG_DATA_HOME", tempDir)
	defer func() { _ = os.Unsetenv("XDG_DATA_HOME") }()

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("Failed to create store: %v", err)
	}

	// Create test scratches in different projects
	project1 := "test-project-1"
	project2 := "test-project-2"
	globalProject := "global"

	// Add scratches to project1
	for i := 0; i < 3; i++ {
		scratch := store.Scratch{
			ID:      generateTestID("proj1", i),
			Project: project1,
			Title:   "Project 1 scratch",
		}
		if err := s.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
		// Create the file
		createTestScratchFile(t, scratch.ID, "content")
	}

	// Add scratches to project2
	for i := 0; i < 2; i++ {
		scratch := store.Scratch{
			ID:      generateTestID("proj2", i),
			Project: project2,
			Title:   "Project 2 scratch",
		}
		if err := s.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
		createTestScratchFile(t, scratch.ID, "content")
	}

	// Add global scratches
	for i := 0; i < 4; i++ {
		scratch := store.Scratch{
			ID:      generateTestID("global", i),
			Project: globalProject,
			Title:   "Global scratch",
		}
		if err := s.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
		createTestScratchFile(t, scratch.ID, "content")
	}

	// Test 1: Nuke specific project
	t.Run("NukeProject", func(t *testing.T) {
		result, err := Nuke(s, false, project1)
		if err != nil {
			t.Fatalf("Failed to nuke project: %v", err)
		}
		if result.DeletedCount != 3 {
			t.Errorf("Expected 3 deleted, got %d", result.DeletedCount)
		}
		if result.Scope != "project" {
			t.Errorf("Expected scope 'project', got %s", result.Scope)
		}
		if result.ProjectName != project1 {
			t.Errorf("Expected project name %s, got %s", project1, result.ProjectName)
		}

		// Verify only project1 scratches were deleted
		remaining := s.GetScratches()
		if len(remaining) != 6 { // 2 from project2 + 4 global
			t.Errorf("Expected 6 remaining scratches, got %d", len(remaining))
		}
	})

	// Test 2: Nuke global
	t.Run("NukeGlobal", func(t *testing.T) {
		result, err := Nuke(s, false, "")
		if err != nil {
			t.Fatalf("Failed to nuke global: %v", err)
		}
		if result.DeletedCount != 4 {
			t.Errorf("Expected 4 deleted, got %d", result.DeletedCount)
		}
		if result.Scope != "global" {
			t.Errorf("Expected scope 'global', got %s", result.Scope)
		}

		// Verify only global scratches were deleted
		remaining := s.GetScratches()
		if len(remaining) != 2 { // 2 from project2
			t.Errorf("Expected 2 remaining scratches, got %d", len(remaining))
		}
	})

	// Test 3: Nuke all
	t.Run("NukeAll", func(t *testing.T) {
		// Re-add some scratches
		if err := s.AddScratch(store.Scratch{ID: generateTestID("new", 0), Project: "new-project", Title: "New"}); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}
		if err := s.AddScratch(store.Scratch{ID: generateTestID("new", 1), Project: globalProject, Title: "New global"}); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}

		result, err := Nuke(s, true, "")
		if err != nil {
			t.Fatalf("Failed to nuke all: %v", err)
		}
		if result.DeletedCount != 4 { // 2 from project2 + 2 new
			t.Errorf("Expected 4 deleted, got %d", result.DeletedCount)
		}
		if result.Scope != "all" {
			t.Errorf("Expected scope 'all', got %s", result.Scope)
		}

		// Verify all scratches were deleted
		remaining := s.GetScratches()
		if len(remaining) != 0 {
			t.Errorf("Expected 0 remaining scratches, got %d", len(remaining))
		}
	})

	// Test 4: Nuke empty project
	t.Run("NukeEmptyProject", func(t *testing.T) {
		result, err := Nuke(s, false, "non-existent-project")
		if err != nil {
			t.Fatalf("Failed to nuke empty project: %v", err)
		}
		if result.DeletedCount != 0 {
			t.Errorf("Expected 0 deleted, got %d", result.DeletedCount)
		}
	})
}

func generateTestID(prefix string, index int) string {
	return fmt.Sprintf("%s_test_%d", prefix, index)
}

func createTestScratchFile(t *testing.T, id, content string) {
	path, err := store.GetScratchFilePath(id)
	if err != nil {
		t.Fatalf("Failed to get scratch file path: %v", err)
	}
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		t.Fatalf("Failed to create directory: %v", err)
	}
	if err := os.WriteFile(path, []byte(content), 0644); err != nil {
		t.Fatalf("Failed to write scratch file: %v", err)
	}
}
