package templates

import (
	_ "embed"
	"fmt"
	"path/filepath"
	"regexp"
	"strings"
	"text/template"
	"time"

	"github.com/arthur-debert/padz/cmd/padz/styles"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/charmbracelet/lipgloss"
	"github.com/charmbracelet/x/term"
	"github.com/dustin/go-humanize"
)

//go:embed pad-list-item.tmpl
var padListItemTemplate string

//go:embed error.tmpl
var errorTemplate string

//go:embed success.tmpl
var successTemplate string

//go:embed empty-state.tmpl
var emptyStateTemplate string

//go:embed search-result.tmpl
var searchResultTemplate string

//go:embed content-view.tmpl
var contentViewTemplate string

//go:embed content-peek.tmpl
var contentPeekTemplate string

type PadListItem struct {
	ID          string
	Title       string
	Project     string
	ProjectName string
	ShowProject bool
	TimeAgo     string
	Index       int
}

type SearchResultItem struct {
	ID               string
	Title            string
	HighlightedTitle string
	Project          string
	ProjectName      string
	ShowProject      bool
	TimeAgo          string
	Index            int
}

type Message struct {
	Message string
}

type ContentView struct {
	Content string
}

type ContentPeek struct {
	StartContent string
	EndContent   string
	HasSkipped   bool
	SkippedLines int
}

type Renderer struct {
	templates map[string]*template.Template
}

func NewRenderer() (*Renderer, error) {
	r := &Renderer{
		templates: make(map[string]*template.Template),
	}

	// Load all templates
	templates := map[string]string{
		"pad-list-item": padListItemTemplate,
		"error":         errorTemplate,
		"success":       successTemplate,
		"empty-state":   emptyStateTemplate,
		"search-result": searchResultTemplate,
		"content-view":  contentViewTemplate,
		"content-peek":  contentPeekTemplate,
	}

	for name, content := range templates {
		tmpl, err := template.New(name).Parse(content)
		if err != nil {
			return nil, fmt.Errorf("failed to parse %s template: %w", name, err)
		}
		r.templates[name] = tmpl
	}

	return r, nil
}

func (r *Renderer) RenderPadListItem(scratch *store.Scratch, showProject bool, index int) (string, error) {
	projectName := ""
	if scratch.Project == "global" {
		projectName = "global"
	} else if scratch.Project != "" {
		projectName = filepath.Base(scratch.Project)
	}

	item := PadListItem{
		ID:          scratch.ID,
		Title:       scratch.Title,
		Project:     scratch.Project,
		ProjectName: projectName,
		ShowProject: showProject,
		TimeAgo:     humanize.Time(scratch.CreatedAt),
		Index:       index,
	}

	var buf strings.Builder
	if err := r.templates["pad-list-item"].Execute(&buf, item); err != nil {
		return "", fmt.Errorf("failed to execute template: %w", err)
	}

	return applyStyles(buf.String()), nil
}

func (r *Renderer) RenderPadList(scratches []*store.Scratch, showProject bool) (string, error) {
	// Get terminal width and calculate column widths
	termWidth := getTerminalWidth()
	widths := calculateColumnWidths(termWidth, showProject)

	var lines []string

	// First pass: render only pinned scratches with p1, p2 indices
	pinnedCount := 0
	hasPinned := false
	for _, scratch := range scratches {
		if !scratch.IsPinned {
			continue
		}
		hasPinned = true
		pinnedCount++

		// Prepare data
		projectName := ""
		if scratch.Project == "global" {
			projectName = "global"
		} else if scratch.Project != "" {
			projectName = filepath.Base(scratch.Project)
		}
		timeAgo := formatTimeAgo(scratch.CreatedAt)

		// Build line with proper column alignment
		var parts []string

		// Pinned index with ⚲ prefix
		indexStr := fmt.Sprintf("p%d.", pinnedCount)
		fullIndexStr := "⚲ " + indexStr
		// Pad based on visible width, not byte length
		padding := widths.ID - lipgloss.Width(fullIndexStr)
		indexPadded := strings.Repeat(" ", padding) + fullIndexStr

		// Project (if showing)
		projectPadded := ""
		if showProject {
			project := truncateWithEllipsis(projectName, widths.Project)
			projectPadded = padRight(project, widths.Project) + "  "
		}

		// Title
		title := truncateWithEllipsis(scratch.Title, widths.Title)
		titlePadded := padRight(title, widths.Title) + "  "

		// Time - with ⚲ prefix for pinned items
		timeStr := "⚲ " + timeAgo
		timePadded := padLeft(timeStr, widths.Date)

		// Apply styles
		indexStyle := styles.Get("padIndex")
		parts = append(parts, strings.Replace(indexPadded, fullIndexStr, indexStyle.Render(fullIndexStr), 1))

		if showProject && projectPadded != "" {
			projectStyle := styles.Get("padProject")
			projectText := strings.TrimSpace(projectPadded[:widths.Project])
			parts = append(parts, strings.Replace(projectPadded, projectText, projectStyle.Render(projectText), 1))
		}

		titleStyle := styles.Get("padTitle")
		titleText := strings.TrimSpace(titlePadded[:widths.Title])
		parts = append(parts, strings.Replace(titlePadded, titleText, titleStyle.Render(titleText), 1))

		timeStyle := styles.Get("padTime")
		// Apply style only to the time part, preserving the ⚲ marker
		styledTime := strings.Replace(timePadded, timeAgo, timeStyle.Render(timeAgo), 1)
		parts = append(parts, styledTime)

		lines = append(lines, strings.Join(parts, ""))
	}

	// Add separator if we have pinned items
	if hasPinned {
		lines = append(lines, "")
		lines = append(lines, "")
	}

	// Second pass: render all scratches with regular indices
	for i, scratch := range scratches {
		// Prepare data
		index := i + 1
		projectName := ""
		if scratch.Project == "global" {
			projectName = "global"
		} else if scratch.Project != "" {
			projectName = filepath.Base(scratch.Project)
		}
		timeAgo := formatTimeAgo(scratch.CreatedAt)

		// Build line with proper column alignment
		var parts []string

		// Regular index
		indexStr := fmt.Sprintf("%d.", index)
		indexPadded := padLeft(indexStr, widths.ID-1) + " "

		// Project (if showing)
		projectPadded := ""
		if showProject {
			project := truncateWithEllipsis(projectName, widths.Project)
			projectPadded = padRight(project, widths.Project) + "  "
		}

		// Title
		title := truncateWithEllipsis(scratch.Title, widths.Title)
		titlePadded := padRight(title, widths.Title) + "  "

		// Time - add pin indicator before if pinned
		timeStr := timeAgo
		if scratch.IsPinned {
			timeStr = "⚲ " + timeAgo
		}
		timePadded := padLeft(timeStr, widths.Date)

		// Apply styles
		indexStyle := styles.Get("padIndex")
		parts = append(parts, strings.Replace(indexPadded, indexStr, indexStyle.Render(indexStr), 1))

		if showProject && projectPadded != "" {
			projectStyle := styles.Get("padProject")
			projectText := strings.TrimSpace(projectPadded[:widths.Project])
			parts = append(parts, strings.Replace(projectPadded, projectText, projectStyle.Render(projectText), 1))
		}

		titleStyle := styles.Get("padTitle")
		titleText := strings.TrimSpace(titlePadded[:widths.Title])
		parts = append(parts, strings.Replace(titlePadded, titleText, titleStyle.Render(titleText), 1))

		timeStyle := styles.Get("padTime")
		// Apply style only to the time part, preserving the ⚲ marker if present
		styledTime := strings.Replace(timePadded, timeAgo, timeStyle.Render(timeAgo), 1)
		parts = append(parts, styledTime)

		lines = append(lines, strings.Join(parts, ""))
	}

	return strings.Join(lines, "\n"), nil
}

// Column width definitions
type columnWidths struct {
	ID      int
	Project int
	Date    int
	Title   int
}

// getTerminalWidth returns the terminal width, bounded between 80 and 120
func getTerminalWidth() int {
	width, _, err := term.GetSize(0)
	if err != nil {
		width = 80
	}

	if width < 80 {
		return 80
	}
	if width > 120 {
		return 120
	}
	return width
}

// calculateColumnWidths determines the width for each column
func calculateColumnWidths(termWidth int, showProject bool) columnWidths {
	widths := columnWidths{
		ID:   7,  // "⚲ p99. " (⚲ + space + p + 2 digits + dot + space)
		Date: 20, // "a long while ago ⚲"
	}

	if showProject {
		widths.Project = 14
	}

	// Calculate title width: terminal - id - date - project - spaces
	spacesCount := 2 // Between ID and title/project
	if showProject {
		spacesCount += 2 // Between project and title
	}
	spacesCount += 2 // Between title and date

	widths.Title = termWidth - widths.ID - widths.Date - widths.Project - spacesCount

	// Ensure title has at least some space
	if widths.Title < 10 {
		widths.Title = 10
	}

	return widths
}

// truncateWithEllipsis truncates a string to maxLen and adds "..." if truncated
func truncateWithEllipsis(s string, maxLen int) string {
	if lipgloss.Width(s) <= maxLen {
		return s
	}

	if maxLen <= 3 {
		return s[:maxLen]
	}

	// Account for "..." when truncating
	truncateAt := maxLen - 3
	runes := []rune(s)
	if truncateAt > len(runes) {
		truncateAt = len(runes)
	}

	return string(runes[:truncateAt]) + "..."
}

// padRight pads a string to the specified width
func padRight(s string, width int) string {
	currentWidth := lipgloss.Width(s)
	if currentWidth >= width {
		return s
	}
	return s + strings.Repeat(" ", width-currentWidth)
}

// padLeft pads a string to the specified width
func padLeft(s string, width int) string {
	currentWidth := lipgloss.Width(s)
	if currentWidth >= width {
		return s
	}
	return strings.Repeat(" ", width-currentWidth) + s
}

// formatTimeAgo formats time with aligned units for better visual alignment
func formatTimeAgo(t time.Time) string {
	timeAgo := humanize.Time(t)

	// Replace single digit numbers with padded versions for alignment
	// This will convert "1 day ago" to " 1 day ago", "2 weeks ago" to " 2 weeks ago", etc.
	re := regexp.MustCompile(`^(\d+)\s+(\w+)\s+ago$`)
	matches := re.FindStringSubmatch(timeAgo)
	if len(matches) == 3 {
		num := matches[1]
		unit := matches[2]

		// Pad single digit numbers
		if len(num) == 1 {
			timeAgo = fmt.Sprintf(" %s %s ago", num, unit)
		}
	}

	return timeAgo
}

func applyStyles(text string) string {
	// Process style tags in the text
	result := text

	// Find all opening tags
	openPattern := regexp.MustCompile(`\[(\w+)\]`)

	for {
		matches := openPattern.FindStringSubmatchIndex(result)
		if matches == nil {
			break
		}

		// Extract tag name
		tagStart := matches[0]
		tagEnd := matches[1]
		nameStart := matches[2]
		nameEnd := matches[3]
		tagName := result[nameStart:nameEnd]

		// Find the corresponding closing tag
		closeTag := "[/" + tagName + "]"
		closeIndex := strings.Index(result[tagEnd:], closeTag)
		if closeIndex == -1 {
			// No matching closing tag, skip this one
			result = result[:tagEnd] + result[tagEnd:]
			continue
		}

		// Extract content between tags
		contentStart := tagEnd
		contentEnd := tagEnd + closeIndex
		content := result[contentStart:contentEnd]

		// Apply style
		style := styles.Get(tagName)
		styled := style.Render(content)

		// Replace the entire tag sequence with styled content
		before := result[:tagStart]
		after := result[contentEnd+len(closeTag):]
		result = before + styled + after
	}

	return result
}

func (r *Renderer) RenderContentView(content string) (string, error) {
	data := ContentView{Content: content}
	var buf strings.Builder
	if err := r.templates["content-view"].Execute(&buf, data); err != nil {
		return "", fmt.Errorf("failed to execute content-view template: %w", err)
	}
	return applyStyles(buf.String()), nil
}

func (r *Renderer) RenderContentPeek(startContent, endContent string, hasSkipped bool, skippedLines int) (string, error) {
	data := ContentPeek{
		StartContent: startContent,
		EndContent:   endContent,
		HasSkipped:   hasSkipped,
		SkippedLines: skippedLines,
	}
	var buf strings.Builder
	if err := r.templates["content-peek"].Execute(&buf, data); err != nil {
		return "", fmt.Errorf("failed to execute content-peek template: %w", err)
	}
	return applyStyles(buf.String()), nil
}
