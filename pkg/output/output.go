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
		// Both plain and term use the same output - terminal detection handles formatting stripping
		for i, scratch := range scratches {
			if showProject {
				projectName := "global"
				if scratch.Project != "global" && scratch.Project != "" {
					projectName = filepath.Base(scratch.Project)
				}
				if _, err := fmt.Fprintf(f.writer, "%d. %s %s %s\n", i+1, projectName, humanize.Time(scratch.CreatedAt), scratch.Title); err != nil {
					return err
				}
			} else {
				if _, err := fmt.Fprintf(f.writer, "%d. %s %s\n", i+1, humanize.Time(scratch.CreatedAt), scratch.Title); err != nil {
					return err
				}
			}
		}
		return nil
	default:
		return fmt.Errorf("unsupported format: %s", f.format)
	}
}

// FormatSearchResults formats search results that include indices
func (f *Formatter) FormatSearchResults(results []commands.ScratchWithIndex, showProject bool) error {
	switch f.format {
	case JSONFormat:
		return json.NewEncoder(f.writer).Encode(results)
	case PlainFormat, TermFormat:
		// Both plain and term use the same output - terminal detection handles formatting stripping
		for _, result := range results {
			if showProject {
				projectName := "global"
				if result.Project != "global" && result.Project != "" {
					projectName = filepath.Base(result.Project)
				}
				if _, err := fmt.Fprintf(f.writer, "%d. %s %s %s\n", result.Index, projectName, humanize.Time(result.CreatedAt), result.Title); err != nil {
					return err
				}
			} else {
				if _, err := fmt.Fprintf(f.writer, "%d. %s %s\n", result.Index, humanize.Time(result.CreatedAt), result.Title); err != nil {
					return err
				}
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
		_, _ = fmt.Fprint(f.writer, content)
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
			if _, err := fmt.Fprintln(f.writer, message); err != nil {
				return err
			}
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
		if _, err := fmt.Fprintln(f.writer, result.Path); err != nil {
			return err
		}
		return nil
	default:
		return fmt.Errorf("unsupported format: %s", f.format)
	}
}
