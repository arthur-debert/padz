package commands

import (
	"fmt"
	"strings"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// AggregateOptions configures how multiple scratch contents are combined
type AggregateOptions struct {
	// IncludeHeaders adds scratch metadata (title, date, etc) before each content
	IncludeHeaders bool

	// Separator is the string used between scratch contents
	Separator string

	// HeaderFormat is a function that formats the header for each scratch
	// If nil, a default format is used
	HeaderFormat func(scratch *store.Scratch, index int) string

	// IncludeEmptyScratches includes scratches with empty content
	IncludeEmptyScratches bool
}

// DefaultSeparators provides common separators for different use cases
var DefaultSeparators = struct {
	Simple    string
	Markdown  string
	Code      string
	Clipboard string
}{
	Simple:    "\n\n",
	Markdown:  "\n\n---\n\n",
	Code:      "\n\n// " + strings.Repeat("-", 60) + "\n\n",
	Clipboard: "\n\n" + strings.Repeat("=", 60) + "\n\n",
}

// DefaultAggregateOptions returns sensible defaults for aggregation
func DefaultAggregateOptions() AggregateOptions {
	return AggregateOptions{
		IncludeHeaders:        false,
		Separator:             DefaultSeparators.Simple,
		HeaderFormat:          nil,
		IncludeEmptyScratches: false,
	}
}

// AggregateOptionsWithHeaders returns options configured for displaying headers
func AggregateOptionsWithHeaders() AggregateOptions {
	return AggregateOptions{
		IncludeHeaders:        true,
		Separator:             DefaultSeparators.Markdown,
		HeaderFormat:          defaultHeaderFormat,
		IncludeEmptyScratches: false,
	}
}

// truncateID safely truncates an ID to 8 characters
func truncateID(id string) string {
	if len(id) > 8 {
		return id[:8]
	}
	return id
}

// defaultHeaderFormat provides the default header formatting
func defaultHeaderFormat(scratch *store.Scratch, index int) string {
	header := fmt.Sprintf("# %s", scratch.Title)

	// Add metadata
	metadata := []string{
		fmt.Sprintf("ID: %s", truncateID(scratch.ID)),
		fmt.Sprintf("Created: %s", scratch.CreatedAt.Format("2006-01-02 15:04:05")),
	}

	if scratch.IsPinned {
		metadata = append(metadata, "📌 Pinned")
	}

	if scratch.UpdatedAt.After(scratch.CreatedAt) {
		metadata = append(metadata, fmt.Sprintf("Updated: %s", scratch.UpdatedAt.Format("2006-01-02 15:04:05")))
	}

	header += "\n" + strings.Join(metadata, " | ")

	return header
}

// AggregatedContent represents the combined content from multiple scratches
type AggregatedContent struct {
	Scratches []*store.Scratch
	Contents  []string
	Options   AggregateOptions
}

// GetCombinedContent returns the raw combined content without any headers
func (ac *AggregatedContent) GetCombinedContent() string {
	return ac.getCombinedContent(false)
}

// GetCombinedContentWithHeaders returns the combined content with headers
func (ac *AggregatedContent) GetCombinedContentWithHeaders() string {
	return ac.getCombinedContent(true)
}

// getCombinedContent is the internal method that handles the actual combination
func (ac *AggregatedContent) getCombinedContent(includeHeaders bool) string {
	if len(ac.Scratches) == 0 {
		return ""
	}

	var parts []string

	for i, scratch := range ac.Scratches {
		content := ac.Contents[i]

		// Skip empty scratches if configured
		if !ac.Options.IncludeEmptyScratches && strings.TrimSpace(content) == "" {
			continue
		}

		var part string

		if includeHeaders && ac.Options.HeaderFormat != nil {
			header := ac.Options.HeaderFormat(scratch, i)
			part = header + "\n\n" + content
		} else {
			part = content
		}

		parts = append(parts, part)
	}

	if len(parts) == 0 {
		return ""
	}

	return strings.Join(parts, ac.Options.Separator)
}

// AggregateScratchContents reads and aggregates content from multiple scratches
func AggregateScratchContents(scratches []*store.Scratch, options AggregateOptions) (*AggregatedContent, error) {
	if len(scratches) == 0 {
		return &AggregatedContent{
			Scratches: scratches,
			Contents:  []string{},
			Options:   options,
		}, nil
	}

	contents := make([]string, len(scratches))

	for i, scratch := range scratches {
		content, err := readScratchFile(scratch.ID)
		if err != nil {
			return nil, fmt.Errorf("failed to read scratch %s: %w", scratch.ID[:8], err)
		}
		contents[i] = string(content)
	}

	return &AggregatedContent{
		Scratches: scratches,
		Contents:  contents,
		Options:   options,
	}, nil
}

// AggregateScratchContentsByIDs resolves IDs and aggregates their content
func AggregateScratchContentsByIDs(s *store.Store, global bool, project string, ids []string, options AggregateOptions) (*AggregatedContent, error) {
	// Resolve all IDs to scratches
	scratches, err := ResolveMultipleIDs(s, global, project, ids)
	if err != nil {
		return nil, err
	}

	return AggregateScratchContents(scratches, options)
}

// FormatScratchSummary creates a summary line for a scratch
func FormatScratchSummary(scratch *store.Scratch) string {
	age := time.Since(scratch.CreatedAt)
	ageStr := formatDuration(age)

	summary := fmt.Sprintf("%s (%s ago)", scratch.Title, ageStr)

	if scratch.IsPinned {
		summary = "📌 " + summary
	}

	return summary
}

// formatDuration formats a duration in a human-readable way
func formatDuration(d time.Duration) string {
	if d < time.Minute {
		return "just now"
	} else if d < time.Hour {
		mins := int(d.Minutes())
		if mins == 1 {
			return "1 minute"
		}
		return fmt.Sprintf("%d minutes", mins)
	} else if d < 24*time.Hour {
		hours := int(d.Hours())
		if hours == 1 {
			return "1 hour"
		}
		return fmt.Sprintf("%d hours", hours)
	} else {
		days := int(d.Hours() / 24)
		if days == 1 {
			return "1 day"
		}
		return fmt.Sprintf("%d days", days)
	}
}
