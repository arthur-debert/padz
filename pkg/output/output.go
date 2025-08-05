package output

import (
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/dustin/go-humanize"
)

// Format represents the output format type
type Format string

const (
	PlainFormat Format = "plain"
	JSONFormat  Format = "json"
	TermFormat  Format = "term"
)

// GetFormat returns the format from string
func GetFormat(s string) (Format, error) {
	switch s {
	case "plain":
		return PlainFormat, nil
	case "json":
		return JSONFormat, nil
	case "term":
		return TermFormat, nil
	default:
		return "", fmt.Errorf("invalid format: %s (valid: plain, json, term)", s)
	}
}

// Formatter handles output formatting
type Formatter struct {
	format Format
	writer io.Writer
}

// NewFormatter creates a new formatter
func NewFormatter(format Format, writer io.Writer) *Formatter {
	if writer == nil {
		writer = os.Stdout
	}
	return &Formatter{
		format: format,
		writer: writer,
	}
}

// FormatList formats a list of scratches
func (f *Formatter) FormatList(scratches []store.Scratch, showProject bool) error {
	switch f.format {
	case JSONFormat:
		return json.NewEncoder(f.writer).Encode(scratches)
	case PlainFormat, TermFormat:
		// For now, term is same as plain
		for _, scratch := range scratches {
			if showProject {
				projectName := "global"
				if scratch.Project != "global" && scratch.Project != "" {
					projectName = filepath.Base(scratch.Project)
				}
				fmt.Fprintf(f.writer, "%s %s %s\n", projectName, humanize.Time(scratch.CreatedAt), scratch.Title)
			} else {
				fmt.Fprintf(f.writer, "%s %s\n", humanize.Time(scratch.CreatedAt), scratch.Title)
			}
		}
		return nil
	default:
		return fmt.Errorf("unsupported format: %s", f.format)
	}
}

// FormatString formats a string output
func (f *Formatter) FormatString(content string) error {
	switch f.format {
	case JSONFormat:
		return json.NewEncoder(f.writer).Encode(map[string]string{"content": content})
	case PlainFormat, TermFormat:
		fmt.Fprint(f.writer, content)
		return nil
	default:
		return fmt.Errorf("unsupported format: %s", f.format)
	}
}

// FormatError formats an error
func (f *Formatter) FormatError(err error) error {
	if err == nil {
		return nil
	}
	
	switch f.format {
	case JSONFormat:
		return json.NewEncoder(f.writer).Encode(map[string]string{"error": err.Error()})
	case PlainFormat, TermFormat:
		// Errors go to stderr in plain/term mode
		fmt.Fprintln(os.Stderr, err.Error())
		return nil
	default:
		return fmt.Errorf("unsupported format: %s", f.format)
	}
}

// FormatSuccess formats a success message
func (f *Formatter) FormatSuccess(message string) error {
	switch f.format {
	case JSONFormat:
		return json.NewEncoder(f.writer).Encode(map[string]string{"success": message})
	case PlainFormat, TermFormat:
		if message != "" {
			fmt.Fprintln(f.writer, message)
		}
		return nil
	default:
		return fmt.Errorf("unsupported format: %s", f.format)
	}
}

// FormatPath formats a path result
func (f *Formatter) FormatPath(result *commands.PathResult) error {
	switch f.format {
	case JSONFormat:
		return json.NewEncoder(f.writer).Encode(result)
	case PlainFormat, TermFormat:
		fmt.Fprintln(f.writer, result.Path)
		return nil
	default:
		return fmt.Errorf("unsupported format: %s", f.format)
	}
}

