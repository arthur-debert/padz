package templates

import (
	_ "embed"
	"fmt"
	"html/template"
	"regexp"
	"strings"

	"github.com/arthur-debert/padz/cmd/padz/styles"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/dustin/go-humanize"
)

//go:embed pad-list-item.tmpl
var padListItemTemplate string

type PadListItem struct {
	ID       string
	Title    string
	Project  string
	TimeAgo  string
}

type Renderer struct {
	templates map[string]*template.Template
}

func NewRenderer() (*Renderer, error) {
	r := &Renderer{
		templates: make(map[string]*template.Template),
	}

	tmpl, err := template.New("pad-list-item").Parse(padListItemTemplate)
	if err != nil {
		return nil, fmt.Errorf("failed to parse pad-list-item template: %w", err)
	}
	r.templates["pad-list-item"] = tmpl

	return r, nil
}

func (r *Renderer) RenderPadListItem(scratch *store.Scratch) (string, error) {
	item := PadListItem{
		ID:      scratch.ID,
		Title:   scratch.Title,
		Project: scratch.Project,
		TimeAgo: humanize.Time(scratch.CreatedAt),
	}

	var buf strings.Builder
	if err := r.templates["pad-list-item"].Execute(&buf, item); err != nil {
		return "", fmt.Errorf("failed to execute template: %w", err)
	}

	return applyStyles(buf.String()), nil
}

func (r *Renderer) RenderPadList(scratches []*store.Scratch) (string, error) {
	var lines []string
	for _, scratch := range scratches {
		line, err := r.RenderPadListItem(scratch)
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