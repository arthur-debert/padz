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
			Content: "content",
		}
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
	}

	// Add scratches to project2
	for i := 0; i < 2; i++ {
		scratch := store.Scratch{
			ID:      generateTestID("proj2", i),
			Project: project2,
			Title:   "Project 2 scratch",
			Content: "content",
		}
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
	}

	// Add global scratches
	for i := 0; i < 4; i++ {
		scratch := store.Scratch{
			ID:      generateTestID("global", i),
			Project: globalProject,
			Title:   "Global scratch",
			Content: "content",
		}
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("Failed to add scratch: %v", err)
		}
	}

	// Test 1: Nuke specific project
	t.Run("NukeProject", func(t *testing.T) {
		result, err := Nuke(setup.Store, project1)
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

		// Verify project1 scratches were soft-deleted
		allScratches := setup.Store.GetAllScratches()
		activeScratches := setup.Store.GetScratches()
		if len(activeScratches) != 6 { // 2 from project2 + 4 global
			t.Errorf("Expected 6 active scratches, got %d", len(activeScratches))
		}

		// Count soft-deleted scratches
		deletedCount := 0
		for _, s := range allScratches {
			if s.Project == project1 && s.IsDeleted {
				deletedCount++
			}
		}
		if deletedCount != 3 {
			t.Errorf("Expected 3 soft-deleted project1 scratches, got %d", deletedCount)
		}
	})

	// Test 2: Nuke global
	t.Run("NukeGlobal", func(t *testing.T) {
		result, err := Nuke(setup.Store, "")
		if err != nil {
			t.Fatalf("Failed to nuke global: %v", err)
		}
		if result.DeletedCount != 4 {
			t.Errorf("Expected 4 deleted, got %d", result.DeletedCount)
		}
		if result.Scope != "global" {
			t.Errorf("Expected scope 'global', got %s", result.Scope)
		}

		// Verify global scratches were soft-deleted
		deletedScratches := setup.Store.GetDeletedScratches()
		globalDeletedCount := 0
		for _, s := range deletedScratches {
			if s.Project == globalProject {
				globalDeletedCount++
			}
		}
		if globalDeletedCount != 4 {
			t.Errorf("Expected 4 soft-deleted global scratches, got %d", globalDeletedCount)
		}

		// Verify only project2 scratches remain active
		activeScratches := setup.Store.GetScratches()
		if len(activeScratches) != 2 { // only 2 from project2
			t.Errorf("Expected 2 active scratches, got %d", len(activeScratches))
		}
	})

	// Test 3: Nuke global again with new scratches
	t.Run("NukeGlobalAgain", func(t *testing.T) {
		// Re-add some scratches
		if err := setup.Store.AddScratch(store.Scratch{ID: generateTestID("new", 0), Project: "new-project", Title: "New", Content: "new content"}); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}
		if err := setup.Store.AddScratch(store.Scratch{ID: generateTestID("new", 1), Project: globalProject, Title: "New global", Content: "new global content"}); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}

		result, err := Nuke(setup.Store, "")
		if err != nil {
			t.Fatalf("Failed to nuke global: %v", err)
		}
		if result.DeletedCount != 1 { // Only the new global scratch (not already deleted)
			t.Errorf("Expected 1 deleted, got %d", result.DeletedCount)
		}
		if result.Scope != "global" {
			t.Errorf("Expected scope 'global', got %s", result.Scope)
		}

		// Verify scratches state after global nuke
		activeScratches := setup.Store.GetScratches()
		if len(activeScratches) != 3 { // 1 new-project + 2 project2
			t.Errorf("Expected 3 active scratches, got %d", len(activeScratches))
		}

		// Verify we have the right active scratches
		projectCount := make(map[string]int)
		for _, s := range activeScratches {
			projectCount[s.Project]++
		}
		if projectCount["new-project"] != 1 || projectCount[project2] != 2 {
			t.Errorf("Expected 1 new-project and 2 project2 scratches, got %v", projectCount)
		}
	})

	// Test 4: Nuke empty project
	t.Run("NukeEmptyProject", func(t *testing.T) {
		result, err := Nuke(setup.Store, "non-existent-project")
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
