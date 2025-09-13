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
			ID:        "test-scratch-1",
			Title:     "Test Scratch 1",
			Project:   "test-project",
			CreatedAt: now.Add(-2 * time.Hour), // older scratch
		},
		{
			ID:        "test-scratch-2",
			Title:     "Test Scratch 2",
			Project:   "test-project",
			CreatedAt: now, // newer scratch, will be index 1
		},
		{
			ID:        "other-project-scratch",
			Title:     "Other Project Scratch",
			Project:   "other-project",
			CreatedAt: now.Add(-1 * time.Hour),
		},
	}

	// Save the test scratches
	if err := s.SaveScratches(testScratches); err != nil {
		t.Fatalf("failed to save scratches: %v", err)
	}

	t.Run("ValidIndex", func(t *testing.T) {
		result, err := Path(s, false, "test-project", "1")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// With reverse chronological order, index 1 should be test-scratch-2 (newest)
		if !strings.Contains(result.Path, "test-scratch-2") {
			t.Errorf("expected path to contain scratch ID 'test-scratch-2', got %s", result.Path)
		}
	})

	t.Run("SecondIndex", func(t *testing.T) {
		result, err := Path(s, false, "test-project", "2")
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		// With reverse chronological order, index 2 should be test-scratch-1 (older)
		if !strings.Contains(result.Path, "test-scratch-1") {
			t.Errorf("expected path to contain scratch ID 'test-scratch-1', got %s", result.Path)
		}
	})

	t.Run("IndexOutOfRange", func(t *testing.T) {
		_, err := Path(s, false, "test-project", "3")
		if err == nil {
			t.Error("expected error for out of range index")
		}
		if !strings.Contains(err.Error(), "out of range") {
			t.Errorf("expected 'out of range' error, got: %v", err)
		}
	})

	t.Run("InvalidIndex", func(t *testing.T) {
		_, err := Path(s, false, "test-project", "invalid")
		if err == nil {
			t.Error("expected error for invalid index")
		}
		if !strings.Contains(err.Error(), "invalid index") {
			t.Errorf("expected 'invalid index' error, got: %v", err)
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
		// The centralized function returns "index out of range" when there are no scratches
		if !strings.Contains(err.Error(), "out of range") {
			t.Errorf("expected 'out of range' error, got: %v", err)
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
		// The centralized function returns "index out of range" when no scratches match the filter
		if !strings.Contains(err.Error(), "out of range") {
			t.Errorf("expected 'out of range' error, got: %v", err)
		}
	})
}
