package commands

import (
	"fmt"
	"testing"

	"github.com/arthur-debert/padz/pkg/store"
)

func TestNuke(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

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
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
		// Create the file
		setup.WriteScratchFile(t, scratch.ID, []byte("content"))
	}

	// Add scratches to project2
	for i := 0; i < 2; i++ {
		scratch := store.Scratch{
			ID:      generateTestID("proj2", i),
			Project: project2,
			Title:   "Project 2 scratch",
		}
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
		setup.WriteScratchFile(t, scratch.ID, []byte("content"))
	}

	// Add global scratches
	for i := 0; i < 4; i++ {
		scratch := store.Scratch{
			ID:      generateTestID("global", i),
			Project: globalProject,
			Title:   "Global scratch",
		}
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
		setup.WriteScratchFile(t, scratch.ID, []byte("content"))
	}

	// Test 1: Nuke specific project
	t.Run("NukeProject", func(t *testing.T) {
		result, err := Nuke(setup.Store, false, project1)
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
		remaining := setup.Store.GetScratches()
		if len(remaining) != 6 { // 2 from project2 + 4 global
			t.Errorf("Expected 6 remaining scratches, got %d", len(remaining))
		}
	})

	// Test 2: Nuke global
	t.Run("NukeGlobal", func(t *testing.T) {
		result, err := Nuke(setup.Store, false, "")
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
		remaining := setup.Store.GetScratches()
		if len(remaining) != 2 { // 2 from project2
			t.Errorf("Expected 2 remaining scratches, got %d", len(remaining))
		}
	})

	// Test 3: Nuke all
	t.Run("NukeAll", func(t *testing.T) {
		// Re-add some scratches
		if err := setup.Store.AddScratch(store.Scratch{ID: generateTestID("new", 0), Project: "new-project", Title: "New"}); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}
		if err := setup.Store.AddScratch(store.Scratch{ID: generateTestID("new", 1), Project: globalProject, Title: "New global"}); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}

		result, err := Nuke(setup.Store, true, "")
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
		remaining := setup.Store.GetScratches()
		if len(remaining) != 0 {
			t.Errorf("Expected 0 remaining scratches, got %d", len(remaining))
		}
	})

	// Test 4: Nuke empty project
	t.Run("NukeEmptyProject", func(t *testing.T) {
		result, err := Nuke(setup.Store, false, "non-existent-project")
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
