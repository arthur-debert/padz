package editor

import (
	"bytes"
	"os"
	"os/exec"
	"strings"
	"testing"
)

func TestOpenInEditor_DefaultEditor(t *testing.T) {
	tests := []struct {
		name           string
		editorEnv      string
		expectEditor   string
		shouldTestExec bool
	}{
		{
			name:           "EDITOR environment variable set",
			editorEnv:      "nano",
			expectEditor:   "nano",
			shouldTestExec: false,
		},
		{
			name:           "EDITOR environment variable empty - default to vim",
			editorEnv:      "",
			expectEditor:   "vim",
			shouldTestExec: false,
		},
		{
			name:           "EDITOR set to custom editor",
			editorEnv:      "/usr/bin/emacs",
			expectEditor:   "/usr/bin/emacs",
			shouldTestExec: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			oldEditor := os.Getenv("EDITOR")
			defer os.Setenv("EDITOR", oldEditor)

			if tt.editorEnv == "" {
				os.Unsetenv("EDITOR")
			} else {
				os.Setenv("EDITOR", tt.editorEnv)
			}

			if !tt.shouldTestExec {
				t.Skipf("Skipping execution test for %s - would require actual editor", tt.expectEditor)
			}
		})
	}
}

func TestOpenInEditor_FileHandling(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	defer os.Setenv("EDITOR", oldEditor)

	testEditor := createMockEditor(t)
	defer os.Remove(testEditor)

	os.Setenv("EDITOR", testEditor)

	tests := []struct {
		name           string
		initialContent []byte
		expectedContent []byte
	}{
		{
			name:           "empty initial content",
			initialContent: []byte{},
			expectedContent: []byte("mock editor output\n"),
		},
		{
			name:           "with initial content",
			initialContent: []byte("initial content"),
			expectedContent: []byte("mock editor output\n"),
		},
		{
			name:           "multiline initial content",
			initialContent: []byte("line 1\nline 2\nline 3"),
			expectedContent: []byte("mock editor output\n"),
		},
		{
			name:           "content with special characters",
			initialContent: []byte("content with !@#$%^&*()"),
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
				os.Setenv("EDITOR", "/nonexistent/editor")
			},
			tearDown: func() {
				os.Unsetenv("EDITOR")
			},
			expectErr: true,
		},
		{
			name: "editor that fails",
			setupFunc: func() {
				failingEditor := createFailingMockEditor(t)
				os.Setenv("EDITOR", failingEditor)
			},
			tearDown: func() {
				editor := os.Getenv("EDITOR")
				os.Remove(editor)
				os.Unsetenv("EDITOR")
			},
			expectErr: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			oldEditor := os.Getenv("EDITOR")
			defer os.Setenv("EDITOR", oldEditor)

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
	defer os.Setenv("EDITOR", oldEditor)

	testEditor := createMockEditor(t)
	defer os.Remove(testEditor)

	os.Setenv("EDITOR", testEditor)

	tempFilesBefore := countTempFiles(t)

	_, err := OpenInEditor([]byte("test content"))
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	tempFilesAfter := countTempFiles(t)

	if tempFilesAfter > tempFilesBefore {
		t.Errorf("temp files not cleaned up: before=%d, after=%d", tempFilesBefore, tempFilesAfter)
	}
}

func TestOpenInEditor_LargeContent(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	defer os.Setenv("EDITOR", oldEditor)

	testEditor := createMockEditor(t)
	defer os.Remove(testEditor)

	os.Setenv("EDITOR", testEditor)

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
	defer os.Setenv("EDITOR", oldEditor)

	testEditor := createMockEditor(t)
	defer os.Remove(testEditor)

	os.Setenv("EDITOR", testEditor)

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
	defer os.Setenv("EDITOR", oldEditor)

	os.Setenv("EDITOR", "true")

	content := []byte("benchmark test content")

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		OpenInEditor(content)
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

func countTempFiles(t *testing.T) int {
	tmpDir := os.TempDir()
	entries, err := os.ReadDir(tmpDir)
	if err != nil {
		t.Logf("couldn't read temp dir: %v", err)
		return 0
	}

	count := 0
	for _, entry := range entries {
		if strings.HasPrefix(entry.Name(), "scratch-") {
			count++
		}
	}
	return count
}

func TestOpenInEditor_EditorWithArguments(t *testing.T) {
	oldEditor := os.Getenv("EDITOR")
	defer os.Setenv("EDITOR", oldEditor)

	mockScript := createMockEditor(t)
	defer os.Remove(mockScript)

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
			os.Setenv("EDITOR", tt.editorCmd)

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
		os.Setenv("EDITOR", oldEditor)
		os.Setenv("PATH", oldPath)
	}()

	os.Setenv("EDITOR", "true")

	_, err := OpenInEditor([]byte("test"))
	if err != nil {
		_, pathErr := exec.LookPath("true")
		if pathErr != nil {
			t.Skip("'true' command not available in PATH")
		}
		t.Errorf("unexpected error with 'true' command: %v", err)
	}
}