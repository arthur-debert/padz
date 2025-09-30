package commands

import (
	"strings"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

func TestPath(t *testing.T) {
	// Setup test environment
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	s := setup.Store

	// Add test scratches to the store with distinct timestamps
	now := time.Now()
	testScratches := []store.Scratch{
		{
			Title:     "Test Scratch 1",
			Project:   "test-project",
			Content:   "content 1",
			CreatedAt: now.Add(-2 * time.Hour), // older scratch
		},
		{
			Title:     "Test Scratch 2",
			Project:   "test-project",
			Content:   "content 2",
			CreatedAt: now, // newer scratch, will be index 1
		},
		{
			Title:     "Other Project Scratch",
			Project:   "other-project",
			Content:   "other content",
			CreatedAt: now.Add(-1 * time.Hour),
		},
	}

	// Get test store for custom timestamps
	testStore, ok := s.GetTestStore()
	if ok {
		// Add each scratch with its specific timestamp
		for i, scratch := range testScratches {
			testStore.SetTimeFunc(func() time.Time { return testScratches[i].CreatedAt })
			if err := s.AddScratch(scratch); err != nil {
				t.Fatalf("failed to add scratch: %v", err)
			}
		}
		testStore.SetTimeFunc(time.Now)
	} else {
		// Fallback to SaveScratches if test store not available
		if err := s.SaveScratches(testScratches); err != nil {
			t.Fatalf("failed to save scratches: %v", err)
		}
	}

	t.Run("ValidIndex", func(t *testing.T) {
		result, err := Path(s, false, "test-project", "1")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Path should be in format nanostore://scratch/<uuid>
		if !strings.HasPrefix(result.Path, "nanostore://scratch/") {
			t.Errorf("expected path to start with 'nanostore://scratch/', got %s", result.Path)
		}
	})

	t.Run("SecondScratch", func(t *testing.T) {
		// Get the actual SimpleIDs assigned to test-project scratches
		scratches := s.GetScratchesWithFilter("test-project", false)
		if len(scratches) < 2 {
			t.Fatalf("expected at least 2 scratches in test-project, got %d", len(scratches))
		}

		// Use the actual SimpleID of the second scratch in test-project
		secondSimpleID := scratches[1].ID
		result, err := Path(s, false, "test-project", secondSimpleID)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// Path should be in format nanostore://scratch/<uuid>
		if !strings.HasPrefix(result.Path, "nanostore://scratch/") {
			t.Errorf("expected path to start with 'nanostore://scratch/', got %s", result.Path)
		}
	})

	t.Run("NonExistentSimpleID", func(t *testing.T) {
		_, err := Path(s, false, "test-project", "999")
		if err == nil {
			t.Error("expected error for non-existent SimpleID")
		}
		if !strings.Contains(err.Error(), "scratch not found") {
			t.Errorf("expected 'scratch not found' error, got: %v", err)
		}
	})

	t.Run("InvalidIndex", func(t *testing.T) {
		_, err := Path(s, false, "test-project", "invalid")
		if err == nil {
			t.Error("expected error for invalid index")
		}
		if !strings.Contains(err.Error(), "scratch not found") {
			t.Errorf("expected 'scratch not found' error, got: %v", err)
		}
	})

	t.Run("ZeroIndex", func(t *testing.T) {
		_, err := Path(s, false, "test-project", "0")
		if err == nil {
			t.Error("expected error for zero index")
		}
	})

	t.Run("NoScratches", func(t *testing.T) {
		// Clear all scratches
		if err := s.SaveScratches([]store.Scratch{}); err != nil {
			t.Fatalf("failed to clear scratches: %v", err)
		}

		_, err := Path(s, false, "test-project", "1")
		if err == nil {
			t.Error("expected error when no scratches found")
		}
		// With nanostore, we get "scratch not found" when SimpleID doesn't exist
		if !strings.Contains(err.Error(), "scratch not found") {
			t.Errorf("expected 'scratch not found' error, got: %v", err)
		}

		// Restore scratches for other tests
		if err := s.SaveScratches(testScratches); err != nil {
			t.Fatalf("failed to restore scratches: %v", err)
		}
	})

	t.Run("WrongProject", func(t *testing.T) {
		_, err := Path(s, false, "nonexistent-project", "1")
		if err == nil {
			t.Error("expected error for nonexistent project")
		}
		// With nanostore, SimpleID "1" exists but belongs to a different project
		if !strings.Contains(err.Error(), "scratch not found in project scope") {
			t.Errorf("expected 'scratch not found in project scope' error, got: %v", err)
		}
	})
}
