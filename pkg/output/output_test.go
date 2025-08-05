package output

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/store"
)

func TestGetFormat(t *testing.T) {
	tests := []struct {
		input    string
		expected Format
		wantErr  bool
	}{
		{"plain", PlainFormat, false},
		{"json", JSONFormat, false},
		{"term", TermFormat, false},
		{"invalid", "", true},
		{"", "", true},
	}

	for _, tt := range tests {
		t.Run(tt.input, func(t *testing.T) {
			got, err := GetFormat(tt.input)
			if tt.wantErr {
				if err == nil {
					t.Error("expected error but got none")
				}
			} else {
				if err != nil {
					t.Errorf("unexpected error: %v", err)
				}
				if got != tt.expected {
					t.Errorf("got %v, want %v", got, tt.expected)
				}
			}
		})
	}
}

func TestFormatListJSON(t *testing.T) {
	scratches := []store.Scratch{
		{
			ID:        "1",
			Title:     "Test Scratch",
			Project:   "test-project",
			CreatedAt: time.Now(),
		},
	}

	buf := new(bytes.Buffer)
	formatter := NewFormatter(JSONFormat, buf)
	
	err := formatter.FormatList(scratches, false)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	// Check if output is valid JSON
	var result []store.Scratch
	if err := json.Unmarshal(buf.Bytes(), &result); err != nil {
		t.Fatalf("output is not valid JSON: %v", err)
	}

	if len(result) != 1 {
		t.Errorf("expected 1 scratch, got %d", len(result))
	}
}

func TestFormatListPlain(t *testing.T) {
	scratches := []store.Scratch{
		{
			ID:        "1",
			Title:     "Test Scratch",
			Project:   "test-project",
			CreatedAt: time.Now().Add(-1 * time.Hour),
		},
	}

	buf := new(bytes.Buffer)
	formatter := NewFormatter(PlainFormat, buf)
	
	err := formatter.FormatList(scratches, false)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	output := buf.String()
	if !strings.Contains(output, "Test Scratch") {
		t.Error("expected title in output")
	}
	if !strings.Contains(output, "hour") {
		t.Error("expected time in output")
	}
}

func TestFormatString(t *testing.T) {
	content := "Hello, World!"

	t.Run("JSON", func(t *testing.T) {
		buf := new(bytes.Buffer)
		formatter := NewFormatter(JSONFormat, buf)
		
		err := formatter.FormatString(content)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		var result map[string]string
		if err := json.Unmarshal(buf.Bytes(), &result); err != nil {
			t.Fatalf("output is not valid JSON: %v", err)
		}

		if result["content"] != content {
			t.Errorf("expected %s, got %s", content, result["content"])
		}
	})

	t.Run("Plain", func(t *testing.T) {
		buf := new(bytes.Buffer)
		formatter := NewFormatter(PlainFormat, buf)
		
		err := formatter.FormatString(content)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if buf.String() != content {
			t.Errorf("expected %s, got %s", content, buf.String())
		}
	})
}

func TestFormatSuccess(t *testing.T) {
	message := "Operation successful"

	t.Run("JSON", func(t *testing.T) {
		buf := new(bytes.Buffer)
		formatter := NewFormatter(JSONFormat, buf)
		
		err := formatter.FormatSuccess(message)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		var result map[string]string
		if err := json.Unmarshal(buf.Bytes(), &result); err != nil {
			t.Fatalf("output is not valid JSON: %v", err)
		}

		if result["success"] != message {
			t.Errorf("expected %s, got %s", message, result["success"])
		}
	})

	t.Run("Plain", func(t *testing.T) {
		buf := new(bytes.Buffer)
		formatter := NewFormatter(PlainFormat, buf)
		
		err := formatter.FormatSuccess(message)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		if !strings.Contains(buf.String(), message) {
			t.Errorf("expected message in output")
		}
	})
}

func TestFormatPath(t *testing.T) {
	pathResult := &commands.PathResult{
		Path: "/tmp/padz/test-scratch-123",
	}

	t.Run("JSON", func(t *testing.T) {
		buf := new(bytes.Buffer)
		formatter := NewFormatter(JSONFormat, buf)
		
		err := formatter.FormatPath(pathResult)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		var result commands.PathResult
		if err := json.Unmarshal(buf.Bytes(), &result); err != nil {
			t.Fatalf("output is not valid JSON: %v", err)
		}

		if result.Path != pathResult.Path {
			t.Errorf("expected path %s, got %s", pathResult.Path, result.Path)
		}
	})

	t.Run("Plain", func(t *testing.T) {
		buf := new(bytes.Buffer)
		formatter := NewFormatter(PlainFormat, buf)
		
		err := formatter.FormatPath(pathResult)
		if err != nil {
			t.Fatalf("unexpected error: %v", err)
		}

		output := strings.TrimSpace(buf.String())
		if output != pathResult.Path {
			t.Errorf("expected %s, got %s", pathResult.Path, output)
		}
	})
}