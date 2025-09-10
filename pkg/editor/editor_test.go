package editor

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"strings"
	"testing"
)

func TestOpenInEditor_WithRealEditor(t *testing.T) {
	// Skip if ed is not available
	if _, err := exec.LookPath("ed"); err != nil {
		t.Skip("ed editor not available in PATH")
	}

	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	// Create a wrapper that uses ed to append a line
	edScript, err := os.CreateTemp("", "ed-test-script-*.txt")
	if err != nil {
		t.Fatalf("failed to create ed script: %v", err)
	}
	defer func() { _ = os.Remove(edScript.Name()) }()

	// Write ed commands to append a test line
	edCommands := `$a
Test line from ed
.
w
q
`
	if _, err := edScript.WriteString(edCommands); err != nil {
		t.Fatalf("failed to write ed script: %v", err)
	}
	if err := edScript.Close(); err != nil {
		t.Fatalf("failed to close ed script: %v", err)
	}

	// Create wrapper script
	wrapper, err := os.CreateTemp("", "ed-wrapper-*.sh")
	if err != nil {
		t.Fatalf("failed to create wrapper: %v", err)
	}
	defer func() { _ = os.Remove(wrapper.Name()) }()

	wrapperScript := fmt.Sprintf(`#!/bin/sh
ed -s "$1" < %s
`, edScript.Name())

	if _, err := wrapper.WriteString(wrapperScript); err != nil {
		t.Fatalf("failed to write wrapper: %v", err)
	}
	if err := wrapper.Close(); err != nil {
		t.Fatalf("failed to close wrapper: %v", err)
	}

	if err := os.Chmod(wrapper.Name(), 0755); err != nil {
		t.Fatalf("failed to make wrapper executable: %v", err)
	}

	_ = os.Setenv("EDITOR", wrapper.Name())

	// Test with initial content
	initialContent := []byte("Initial line 1\nInitial line 2")
	result, err := OpenInEditor(initialContent)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	expectedContent := "Initial line 1\nInitial line 2\nTest line from ed\n"
	if string(result) != expectedContent {
		t.Errorf("content mismatch:\nexpected: %q\ngot: %q", expectedContent, string(result))
	}
}

func TestOpenInEditor_FileHandling(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	testEditor := createMockEditor(t)
	defer func() { _ = os.Remove(testEditor) }()

	_ = os.Setenv("EDITOR", testEditor)

	tests := []struct {
		name            string
		initialContent  []byte
		expectedContent []byte
	}{
		{
			name:            "empty initial content",
			initialContent:  []byte{},
			expectedContent: []byte("mock editor output\n"),
		},
		{
			name:            "with initial content",
			initialContent:  []byte("initial content"),
			expectedContent: []byte("mock editor output\n"),
		},
		{
			name:            "multiline initial content",
			initialContent:  []byte("line 1\nline 2\nline 3"),
			expectedContent: []byte("mock editor output\n"),
		},
		{
			name:            "content with special characters",
			initialContent:  []byte("content with !@#$%^&*()"),
			expectedContent: []byte("mock editor output\n"),
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := OpenInEditor(tt.initialContent)
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			if !bytes.Equal(result, tt.expectedContent) {
				t.Errorf("expected content %q, got %q", string(tt.expectedContent), string(result))
			}
		})
	}
}

func TestOpenInEditor_ErrorHandling(t *testing.T) {
	tests := []struct {
		name      string
		setupFunc func()
		tearDown  func()
		expectErr bool
	}{
		{
			name: "nonexistent editor",
			setupFunc: func() {
				_ = os.Setenv("EDITOR", "/nonexistent/editor")
			},
			tearDown: func() {
				_ = os.Unsetenv("EDITOR")
			},
			expectErr: true,
		},
		{
			name: "editor that fails",
			setupFunc: func() {
				failingEditor := createFailingMockEditor(t)
				_ = os.Setenv("EDITOR", failingEditor)
			},
			tearDown: func() {
				editor := os.Getenv("EDITOR")
				_ = os.Remove(editor)
				_ = os.Unsetenv("EDITOR")
			},
			expectErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			oldEditor := os.Getenv("EDITOR")
			defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

			tt.setupFunc()
			defer tt.tearDown()

			_, err := OpenInEditor([]byte("test"))
			if tt.expectErr && err == nil {
				t.Errorf("expected error but got none")
			}
			if !tt.expectErr && err != nil {
				t.Errorf("unexpected error: %v", err)
			}
		})
	}
}

func TestOpenInEditor_TempFileCleanup(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	// Track the temp file that gets created
	trackerFile, err := os.CreateTemp("", "tracker-*.txt")
	if err != nil {
		t.Fatalf("failed to create tracker file: %v", err)
	}
	trackerPath := trackerFile.Name()
	_ = trackerFile.Close()
	defer func() { _ = os.Remove(trackerPath) }()

	// Create a mock editor that saves the temp file path
	testEditor := fmt.Sprintf(`#!/bin/bash
echo "$1" > %s
echo "mock editor output" > "$1"
`, trackerPath)

	editorPath := createExecutableScript(t, testEditor)
	defer func() { _ = os.Remove(editorPath) }()

	_ = os.Setenv("EDITOR", editorPath)

	_, err = OpenInEditor([]byte("test content"))
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	// Read the temp file path that was passed to the editor
	data, err := os.ReadFile(trackerPath)
	if err != nil {
		t.Fatalf("failed to read tracker file: %v", err)
	}

	tempFilePath := strings.TrimSpace(string(data))

	// Check if the temp file was cleaned up
	if _, err := os.Stat(tempFilePath); err == nil {
		t.Errorf("temp file %s was not cleaned up", tempFilePath)
	} else if !os.IsNotExist(err) {
		t.Errorf("unexpected error checking temp file: %v", err)
	}
}

func TestOpenInEditor_LargeContent(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	testEditor := createMockEditor(t)
	defer func() { _ = os.Remove(testEditor) }()

	_ = os.Setenv("EDITOR", testEditor)

	largeContent := bytes.Repeat([]byte("this is a test line\n"), 10000)

	result, err := OpenInEditor(largeContent)
	if err != nil {
		t.Errorf("unexpected error with large content: %v", err)
	}

	if len(result) == 0 {
		t.Errorf("expected non-empty result for large content")
	}
}

func TestOpenInEditor_BinaryContent(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	testEditor := createMockEditor(t)
	defer func() { _ = os.Remove(testEditor) }()

	_ = os.Setenv("EDITOR", testEditor)

	binaryContent := []byte{0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE, 0xFD}

	result, err := OpenInEditor(binaryContent)
	if err != nil {
		t.Errorf("unexpected error with binary content: %v", err)
	}

	if len(result) == 0 {
		t.Errorf("expected non-empty result for binary content")
	}
}

func BenchmarkOpenInEditor(b *testing.B) {
	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	_ = os.Setenv("EDITOR", "true")

	content := []byte("benchmark test content")

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_, _ = OpenInEditor(content)
	}
}

func createMockEditor(t *testing.T) string {
	content := `#!/bin/bash
echo "mock editor output" > "$1"
`
	return createExecutableScript(t, content)
}

func createFailingMockEditor(t *testing.T) string {
	content := `#!/bin/bash
exit 1
`
	return createExecutableScript(t, content)
}

func createExecutableScript(t *testing.T, content string) string {
	tmpFile, err := os.CreateTemp("", "mockeditor-*.sh")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}

	if _, err := tmpFile.WriteString(content); err != nil {
		t.Fatalf("failed to write script: %v", err)
	}

	if err := tmpFile.Close(); err != nil {
		t.Fatalf("failed to close temp file: %v", err)
	}

	if err := os.Chmod(tmpFile.Name(), 0755); err != nil {
		t.Fatalf("failed to make script executable: %v", err)
	}

	return tmpFile.Name()
}

func TestOpenInEditor_EditorWithArguments(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	mockScript := createMockEditor(t)
	defer func() { _ = os.Remove(mockScript) }()

	tests := []struct {
		name         string
		editorCmd    string
		expectsError bool
	}{
		{
			name:         "simple editor command",
			editorCmd:    mockScript,
			expectsError: false,
		},
		{
			name:         "editor with single argument",
			editorCmd:    mockScript + " -n",
			expectsError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_ = os.Setenv("EDITOR", tt.editorCmd)

			_, err := OpenInEditor([]byte("test"))
			if tt.expectsError && err == nil {
				t.Errorf("expected error for editor with arguments")
			}
			if !tt.expectsError && err != nil {
				t.Errorf("unexpected error: %v", err)
			}
		})
	}
}

func TestOpenInEditor_PathLookup(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	oldPath := os.Getenv("PATH")
	defer func() {
		_ = os.Setenv("EDITOR", oldEditor)
		_ = os.Setenv("PATH", oldPath)
	}()

	_ = os.Setenv("EDITOR", "true")

	_, err := OpenInEditor([]byte("test"))
	if err != nil {
		_, pathErr := exec.LookPath("true")
		if pathErr != nil {
			t.Skip("'true' command not available in PATH")
		}
		t.Errorf("unexpected error with 'true' command: %v", err)
	}
}

func TestOpenInEditorWithExtension_ExtensionHandling(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	// Create a mock editor that records the filename
	editorScript := `#!/bin/bash
echo "$1" > "$1.filename"
echo "mock editor output" > "$1"
`
	mockEditor := createExecutableScript(t, editorScript)
	defer func() { _ = os.Remove(mockEditor) }()

	_ = os.Setenv("EDITOR", mockEditor)

	tests := []struct {
		name          string
		content       []byte
		extensionHint string
		expectedExt   string
	}{
		{
			name:          "explicit .txt extension",
			content:       []byte("plain text"),
			extensionHint: ".txt",
			expectedExt:   ".txt",
		},
		{
			name:          "explicit .md extension",
			content:       []byte("# Markdown"),
			extensionHint: ".md",
			expectedExt:   ".md",
		},
		{
			name:          "auto-detect markdown from content",
			content:       []byte("# This is markdown"),
			extensionHint: "",
			expectedExt:   ".md",
		},
		{
			name:          "auto-detect markdown with spaces",
			content:       []byte("  # This is markdown with spaces"),
			extensionHint: "",
			expectedExt:   ".md",
		},
		{
			name:          "default to txt for plain content",
			content:       []byte("This is plain text"),
			extensionHint: "",
			expectedExt:   ".txt",
		},
		{
			name:          "empty content defaults to txt",
			content:       []byte(""),
			extensionHint: "",
			expectedExt:   ".txt",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			_, err := OpenInEditorWithExtension(tt.content, tt.extensionHint)
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			// Check if a temp file with the expected extension was created
			// by looking at temp files in the system
			tmpDir := os.TempDir()
			entries, err := os.ReadDir(tmpDir)
			if err != nil {
				t.Errorf("couldn't read temp dir: %v", err)
				return
			}

			found := false
			for _, entry := range entries {
				if strings.HasPrefix(entry.Name(), "scratch-") &&
					strings.HasSuffix(entry.Name(), tt.expectedExt+".filename") {
					found = true
					// Cleanup the filename tracker
					_ = os.Remove(tmpDir + "/" + entry.Name())
					break
				}
			}

			if !found {
				t.Errorf("expected temp file with extension %s not found", tt.expectedExt)
			}
		})
	}
}
