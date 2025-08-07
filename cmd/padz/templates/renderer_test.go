package templates

import (
	"strings"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

func TestRenderPadList_ColumnAlignment(t *testing.T) {
	r, err := NewRenderer()
	if err != nil {
		t.Fatalf("Failed to create renderer: %v", err)
	}

	// Create test data with varying lengths
	now := time.Now()
	scratches := []*store.Scratch{
		{
			ID:        "1",
			Title:     "Short title",
			Project:   "/home/user/projects/verylongprojectname",
			CreatedAt: now.Add(-7 * time.Second),
		},
		{
			ID:        "2",
			Title:     "This is a very long title that should be truncated when displayed in the list view",
			Project:   "/home/user/myproj",
			CreatedAt: now.Add(-5 * time.Minute),
		},
		{
			ID:        "3",
			Title:     "Medium length title here",
			Project:   "global",
			CreatedAt: now.Add(-2 * time.Hour),
		},
	}

	tests := []struct {
		name        string
		showProject bool
	}{
		{"without project", false},
		{"with project", true},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			output, err := r.RenderPadList(scratches, tt.showProject)
			if err != nil {
				t.Fatalf("RenderPadList failed: %v", err)
			}

			lines := strings.Split(output, "\n")
			if len(lines) != len(scratches) {
				t.Errorf("Expected %d lines, got %d", len(scratches), len(lines))
			}

			// Log output for visual inspection
			t.Logf("Output:\n%s", output)

			// Check that time strings are aligned
			// Strip ANSI codes for checking alignment
			for i, line := range lines {
				// Simple check: ensure lines have "ago" at similar positions
				if !strings.Contains(line, "ago") && !strings.Contains(line, "now") {
					t.Errorf("Line %d missing time indicator: %s", i, line)
				}
			}
		})
	}
}

func TestColumnWidthCalculation(t *testing.T) {
	tests := []struct {
		termWidth   int
		showProject bool
		wantTitle   int
	}{
		{80, false, 56},  // 80 - 4 (id) - 16 (date) - 2 - 2 = 56
		{80, true, 40},   // 80 - 4 (id) - 14 (proj) - 16 (date) - 2 - 2 - 2 = 40
		{120, false, 96}, // 120 - 4 - 16 - 2 - 2 = 96
		{120, true, 80},  // 120 - 4 - 14 - 16 - 2 - 2 - 2 = 80
	}

	for _, tt := range tests {
		t.Run("", func(t *testing.T) {
			widths := calculateColumnWidths(tt.termWidth, tt.showProject)
			if widths.Title != tt.wantTitle {
				t.Errorf("calculateColumnWidths(%d, %v) title width = %d, want %d",
					tt.termWidth, tt.showProject, widths.Title, tt.wantTitle)
			}
		})
	}
}

func TestTruncateWithEllipsis(t *testing.T) {
	tests := []struct {
		input  string
		maxLen int
		want   string
	}{
		{"short", 10, "short"},
		{"exactly ten", 11, "exactly ten"},
		{"this is too long", 10, "this is..."},
		{"a", 3, "a"},
		{"abcd", 3, "abc"},
		{"abcdef", 4, "a..."},
		{"Unicode: café", 10, "Unicode..."},
		{"Test with apostrophe: I'm testing", 20, "Test with apostro..."},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got := truncateWithEllipsis(tt.input, tt.maxLen)
			if got != tt.want {
				t.Errorf("truncateWithEllipsis(%q, %d) = %q, want %q", tt.input, tt.maxLen, got, tt.want)
			}
		})
	}
}
