package commands

import (
	"strings"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

func TestAggregateScratchContents(t *testing.T) {
	// Create test scratches
	now := time.Now()
	scratches := []*store.Scratch{
		{
			ID:        "abcdef123456",
			Title:     "First Scratch",
			CreatedAt: now.Add(-2 * time.Hour),
			UpdatedAt: now.Add(-1 * time.Hour),
		},
		{
			ID:        "123456abcdef",
			Title:     "Second Scratch",
			CreatedAt: now.Add(-1 * time.Hour),
			UpdatedAt: now.Add(-30 * time.Minute),
			IsPinned:  true,
			PinnedAt:  now.Add(-30 * time.Minute),
		},
	}

	t.Run("GetCombinedContent with simple separator", func(t *testing.T) {
		contents := []string{"Content of first scratch", "Content of second scratch"}
		options := DefaultAggregateOptions()

		aggregated := &AggregatedContent{
			Scratches: scratches,
			Contents:  contents,
			Options:   options,
		}

		combined := aggregated.GetCombinedContent()
		expected := "Content of first scratch\n\nContent of second scratch"

		if combined != expected {
			t.Errorf("Expected:\n%s\nGot:\n%s", expected, combined)
		}
	})

	t.Run("GetCombinedContentWithHeaders", func(t *testing.T) {
		contents := []string{"Content of first scratch", "Content of second scratch"}
		options := AggregateOptionsWithHeaders()

		aggregated := &AggregatedContent{
			Scratches: scratches,
			Contents:  contents,
			Options:   options,
		}

		combined := aggregated.GetCombinedContentWithHeaders()

		// Check that headers are included
		if !strings.Contains(combined, "# First Scratch") {
			t.Error("Expected header for first scratch not found")
		}
		if !strings.Contains(combined, "# Second Scratch") {
			t.Error("Expected header for second scratch not found")
		}

		// Check that metadata is included
		if !strings.Contains(combined, "ID: abcdef12") {
			t.Error("Expected ID in first scratch header")
		}
		if !strings.Contains(combined, "ID: 123456ab") {
			t.Error("Expected ID in second scratch header")
		}

		// Check that pinned indicator is shown
		if !strings.Contains(combined, "📌 Pinned") {
			t.Error("Expected pinned indicator for second scratch")
		}

		// Check that content is included
		if !strings.Contains(combined, "Content of first scratch") {
			t.Error("Expected content of first scratch")
		}
		if !strings.Contains(combined, "Content of second scratch") {
			t.Error("Expected content of second scratch")
		}

		// Check that separator is used
		if !strings.Contains(combined, "---") {
			t.Error("Expected markdown separator")
		}
	})

	t.Run("Custom separator", func(t *testing.T) {
		contents := []string{"Content 1", "Content 2"}
		options := AggregateOptions{
			IncludeHeaders: false,
			Separator:      "\n===CUSTOM===\n",
		}

		aggregated := &AggregatedContent{
			Scratches: scratches,
			Contents:  contents,
			Options:   options,
		}

		combined := aggregated.GetCombinedContent()
		expected := "Content 1\n===CUSTOM===\nContent 2"

		if combined != expected {
			t.Errorf("Expected:\n%s\nGot:\n%s", expected, combined)
		}
	})

	t.Run("Empty scratches handling", func(t *testing.T) {
		// Add a third scratch for this test
		thirdScratch := &store.Scratch{
			ID:        "fedcba654321",
			Title:     "Third Scratch",
			CreatedAt: now.Add(-30 * time.Minute),
			UpdatedAt: now.Add(-15 * time.Minute),
		}
		testScratches := append(scratches, thirdScratch)

		contents := []string{"Content 1", "", "Content 3"}
		options := AggregateOptions{
			IncludeHeaders:        false,
			Separator:             "\n---\n",
			IncludeEmptyScratches: false,
		}

		aggregated := &AggregatedContent{
			Scratches: testScratches,
			Contents:  contents,
			Options:   options,
		}

		combined := aggregated.GetCombinedContent()
		expected := "Content 1\n---\nContent 3"

		if combined != expected {
			t.Errorf("Expected:\n%s\nGot:\n%s", expected, combined)
		}

		// Test with IncludeEmptyScratches = true
		options.IncludeEmptyScratches = true
		aggregated.Options = options

		combined = aggregated.GetCombinedContent()
		expected = "Content 1\n---\n\n---\nContent 3"

		if combined != expected {
			t.Errorf("Expected with empty:\n%s\nGot:\n%s", expected, combined)
		}
	})

	t.Run("Custom header format", func(t *testing.T) {
		contents := []string{"Content 1", "Content 2"}
		customHeaderFormat := func(scratch *store.Scratch, index int) string {
			return "=== " + scratch.Title + " ==="
		}

		options := AggregateOptions{
			IncludeHeaders: true,
			Separator:      "\n\n",
			HeaderFormat:   customHeaderFormat,
		}

		aggregated := &AggregatedContent{
			Scratches: scratches,
			Contents:  contents,
			Options:   options,
		}

		combined := aggregated.GetCombinedContentWithHeaders()

		if !strings.Contains(combined, "=== First Scratch ===") {
			t.Error("Expected custom header for first scratch")
		}
		if !strings.Contains(combined, "=== Second Scratch ===") {
			t.Error("Expected custom header for second scratch")
		}
	})

	t.Run("No scratches", func(t *testing.T) {
		options := DefaultAggregateOptions()
		aggregated := &AggregatedContent{
			Scratches: []*store.Scratch{},
			Contents:  []string{},
			Options:   options,
		}

		combined := aggregated.GetCombinedContent()
		if combined != "" {
			t.Errorf("Expected empty string for no scratches, got: %s", combined)
		}

		combined = aggregated.GetCombinedContentWithHeaders()
		if combined != "" {
			t.Errorf("Expected empty string for no scratches with headers, got: %s", combined)
		}
	})
}

func TestFormatScratchSummary(t *testing.T) {
	now := time.Now()

	tests := []struct {
		name     string
		scratch  *store.Scratch
		expected string
	}{
		{
			name: "Recent scratch",
			scratch: &store.Scratch{
				Title:     "Recent Note",
				CreatedAt: now.Add(-30 * time.Second),
			},
			expected: "Recent Note (just now ago)",
		},
		{
			name: "Scratch from minutes ago",
			scratch: &store.Scratch{
				Title:     "Note from Minutes",
				CreatedAt: now.Add(-5 * time.Minute),
			},
			expected: "Note from Minutes (5 minutes ago)",
		},
		{
			name: "Scratch from hours ago",
			scratch: &store.Scratch{
				Title:     "Note from Hours",
				CreatedAt: now.Add(-3 * time.Hour),
			},
			expected: "Note from Hours (3 hours ago)",
		},
		{
			name: "Pinned scratch",
			scratch: &store.Scratch{
				Title:     "Pinned Note",
				CreatedAt: now.Add(-2 * time.Hour),
				IsPinned:  true,
			},
			expected: "📌 Pinned Note (2 hours ago)",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := FormatScratchSummary(tt.scratch)
			if result != tt.expected {
				t.Errorf("Expected: %s, Got: %s", tt.expected, result)
			}
		})
	}
}

func TestDefaultSeparators(t *testing.T) {
	// Just verify that the default separators are as expected
	if DefaultSeparators.Simple != "\n\n" {
		t.Errorf("Expected Simple separator to be \\n\\n, got %q", DefaultSeparators.Simple)
	}

	if DefaultSeparators.Markdown != "\n\n---\n\n" {
		t.Errorf("Expected Markdown separator to be \\n\\n---\\n\\n, got %q", DefaultSeparators.Markdown)
	}

	if !strings.Contains(DefaultSeparators.Code, "//") {
		t.Errorf("Expected Code separator to contain //, got %q", DefaultSeparators.Code)
	}

	if !strings.Contains(DefaultSeparators.Clipboard, "=") {
		t.Errorf("Expected Clipboard separator to contain =, got %q", DefaultSeparators.Clipboard)
	}
}
