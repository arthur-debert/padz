package output

import (
	"fmt"
	"strings"
)

// Format represents the output format type
type Format int

const (
	// TermFormat is the default rich terminal output with colors and formatting
	TermFormat Format = iota
	// PlainFormat is plain text output without colors or rich formatting
	PlainFormat
	// JSONFormat is structured JSON output
	JSONFormat
)

// String returns the string representation of the format
func (f Format) String() string {
	switch f {
	case TermFormat:
		return "term"
	case PlainFormat:
		return "plain"
	case JSONFormat:
		return "json"
	default:
		return "term"
	}
}

// GetFormat parses a format string and returns the corresponding Format
func GetFormat(formatStr string) (Format, error) {
	switch strings.ToLower(formatStr) {
	case "term", "terminal":
		return TermFormat, nil
	case "plain", "text":
		return PlainFormat, nil
	case "json":
		return JSONFormat, nil
	default:
		return TermFormat, fmt.Errorf("unknown format: %s (supported: term, plain, json)", formatStr)
	}
}
