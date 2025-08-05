package templates

import (
	_ "embed"
	"fmt"
	"path/filepath"
	"regexp"
	"strings"
	"text/template"

	"github.com/arthur-debert/padz/cmd/padz/styles"
	"github.com/arthur-debert/padz/pkg/store"
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
	StartContent  string
	EndContent    string
	HasSkipped    bool
	SkippedLines  int
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
	var lines []string
	for i, scratch := range scratches {
		line, err := r.RenderPadListItem(scratch, showProject, i+1)
		if err != nil {
			return "", err
		}
		lines = append(lines, line)
	}
	return strings.Join(lines, "\n"), nil
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