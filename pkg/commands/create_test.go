package commands

import (
	"bytes"
	"os"
	"testing"
)

func TestCreateWithTitle(t *testing.T) {
	tests := []struct {
		name           string
		project        string
		content        []byte
		providedTitle  string
		expectedTitle  string
		expectError    bool
		expectSave     bool
		expectedEditor bool
	}{
		{
			name:           "create with content and provided title",
			project:        "testproject",
			content:        []byte("Hello World\nThis is a test"),
			providedTitle:  "My Custom Title",
			expectedTitle:  "My Custom Title",
			expectError:    false,
			expectSave:     true,
			expectedEditor: false,
		},
		{
			name:           "create with content but no provided title",
			project:        "testproject",
			content:        []byte("First Line Title\nThis is content"),
			providedTitle:  "",
			expectedTitle:  "First Line Title",
			expectError:    false,
			expectSave:     true,
			expectedEditor: false,
		},
		{
			name:           "create with empty content and provided title - opens editor",
			project:        "testproject",
			content:        []byte{},
			providedTitle:  "Editor Title",
			expectedTitle:  "Editor Title",
			expectError:    false,
			expectSave:     true,
			expectedEditor: true,
		},
		{
			name:           "create with whitespace only content",
			project:        "testproject",
			content:        []byte("   \n\t\n   "),
			providedTitle:  "Whitespace Title",
			expectedTitle:  "",
			expectError:    false,
			expectSave:     false,
			expectedEditor: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			setup := SetupCommandTest(t)
			defer setup.Cleanup()

			if tt.expectedEditor {
				// Set up a mock editor that writes test content
				oldEditor := os.Getenv("EDITOR")
				testScript := createMockEditorScript(t, tt.providedTitle+"\n\nContent from editor")
				defer func() { _ = os.Remove(testScript) }()
				_ = os.Setenv("EDITOR", testScript)
				defer func() {
					if oldEditor == "" {
						_ = os.Unsetenv("EDITOR")
					} else {
						_ = os.Setenv("EDITOR", oldEditor)
					}
				}()
			}

			initialCount := len(setup.Store.GetScratches())

			err := CreateWithTitle(setup.Store, tt.project, tt.content, tt.providedTitle)
			if tt.expectError {
				if err == nil {
					t.Errorf("expected error but got none")
				}
				return
			}
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			scratches := setup.Store.GetScratches()
			if tt.expectSave {
				if len(scratches) != initialCount+1 {
					t.Errorf("expected scratch to be saved, count: %d -> %d", initialCount, len(scratches))
				}
				if len(scratches) > 0 {
					lastScratch := scratches[len(scratches)-1]
					if lastScratch.Project != tt.project {
						t.Errorf("expected project %s, got %s", tt.project, lastScratch.Project)
					}
					if lastScratch.Title != tt.expectedTitle {
						t.Errorf("expected title %q, got %q", tt.expectedTitle, lastScratch.Title)
					}
				}
			} else {
				if len(scratches) != initialCount {
					t.Errorf("expected scratch not to be saved, count: %d -> %d", initialCount, len(scratches))
				}
			}
		})
	}
}

func TestCreateWithTitleAndContent(t *testing.T) {
	tests := []struct {
		name               string
		project            string
		title              string
		initialContent     []byte
		expectedEditorText string
		finalEditorContent string
		expectedTitle      string
		expectError        bool
		expectSave         bool
	}{
		{
			name:               "both title and initial content",
			project:            "testproject",
			title:              "My Title",
			initialContent:     []byte("Initial content"),
			expectedEditorText: "My Title\n\nInitial content",
			finalEditorContent: "My Title\n\nInitial content\nMore content added",
			expectedTitle:      "My Title",
			expectError:        false,
			expectSave:         true,
		},
		{
			name:               "only title provided",
			project:            "testproject",
			title:              "Only Title",
			initialContent:     []byte{},
			expectedEditorText: "Only Title\n\n",
			finalEditorContent: "Only Title\n\nContent added in editor",
			expectedTitle:      "Only Title",
			expectError:        false,
			expectSave:         true,
		},
		{
			name:               "only initial content provided",
			project:            "testproject",
			title:              "",
			initialContent:     []byte("Just content"),
			expectedEditorText: "Just content",
			finalEditorContent: "Just content\nMore lines",
			expectedTitle:      "Just content",
			expectError:        false,
			expectSave:         true,
		},
		{
			name:               "neither title nor content",
			project:            "testproject",
			title:              "",
			initialContent:     []byte{},
			expectedEditorText: "",
			finalEditorContent: "New content from editor",
			expectedTitle:      "New content from editor",
			expectError:        false,
			expectSave:         true,
		},
		{
			name:               "user clears all content in editor",
			project:            "testproject",
			title:              "Title",
			initialContent:     []byte("Content"),
			expectedEditorText: "Title\n\nContent",
			finalEditorContent: "",
			expectedTitle:      "",
			expectError:        false,
			expectSave:         false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			setup := SetupCommandTest(t)
			defer setup.Cleanup()

			// Set up a mock editor that verifies initial content and writes final content
			oldEditor := os.Getenv("EDITOR")
			testScript := createVerifyingMockEditorScript(t, tt.expectedEditorText, tt.finalEditorContent)
			defer func() { _ = os.Remove(testScript) }()
			_ = os.Setenv("EDITOR", testScript)
			defer func() {
				if oldEditor == "" {
					_ = os.Unsetenv("EDITOR")
				} else {
					_ = os.Setenv("EDITOR", oldEditor)
				}
			}()

			initialCount := len(setup.Store.GetScratches())

			err := CreateWithTitleAndContent(setup.Store, tt.project, tt.title, tt.initialContent)
			if tt.expectError {
				if err == nil {
					t.Errorf("expected error but got none")
				}
				return
			}
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			scratches := setup.Store.GetScratches()
			if tt.expectSave {
				if len(scratches) != initialCount+1 {
					t.Errorf("expected scratch to be saved, count: %d -> %d", initialCount, len(scratches))
				}
				if len(scratches) > 0 {
					lastScratch := scratches[len(scratches)-1]
					if lastScratch.Project != tt.project {
						t.Errorf("expected project %s, got %s", tt.project, lastScratch.Project)
					}
					if lastScratch.Title != tt.expectedTitle {
						t.Errorf("expected title %q, got %q", tt.expectedTitle, lastScratch.Title)
					}

					// Verify the saved content matches what the editor returned
					if lastScratch.Content != tt.finalEditorContent {
						t.Errorf("expected saved content %q, got %q", tt.finalEditorContent, lastScratch.Content)
					}
				}
			} else {
				if len(scratches) != initialCount {
					t.Errorf("expected scratch not to be saved, count: %d -> %d", initialCount, len(scratches))
				}
			}
		})
	}
}

func TestCreateWithPipedContentAndTitle(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	pipedContent := "Piped content line 1\nPiped content line 2"
	providedTitle := "Custom Title for Piped Content"

	// Simulate piped content
	reader := bytes.NewBufferString(pipedContent)
	content := ReadContentFromPipeWithReader(reader)

	err := CreateWithTitle(setup.Store, "testproject", content, providedTitle)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	scratches := setup.Store.GetScratches()
	if len(scratches) != 1 {
		t.Fatalf("expected 1 scratch, got %d", len(scratches))
	}

	scratch := scratches[0]
	if scratch.Title != providedTitle {
		t.Errorf("expected title %q, got %q", providedTitle, scratch.Title)
	}

	if scratch.Content != pipedContent {
		t.Errorf("expected saved content %q, got %q", pipedContent, scratch.Content)
	}
}

func createVerifyingMockEditorScript(t *testing.T, expectedContent, outputContent string) string {
	// Create a script that verifies the initial content and writes the output content
	scriptContent := `#!/bin/bash
FILE="$1"

# Read file content preserving trailing newlines
ACTUAL=$(cat "$FILE"; echo -n x)
ACTUAL="${ACTUAL%x}"

# Expected content
EXPECTED="` + expectedContent + `"

# For debugging
echo "Expected: '$EXPECTED'" >&2
echo "Actual: '$ACTUAL'" >&2

if [ "$ACTUAL" != "$EXPECTED" ]; then
    echo "ERROR: Initial content mismatch" >&2
    echo "Expected: '$EXPECTED'" >&2
    echo "Actual: '$ACTUAL'" >&2
    exit 1
fi

cat > "$FILE" << 'EOF'
` + outputContent + `
EOF
`
	tmpFile, err := os.CreateTemp("", "verifying-editor-*.sh")
	if err != nil {
		t.Fatalf("failed to create temp file: %v", err)
	}

	if _, err := tmpFile.WriteString(scriptContent); err != nil {
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
