package commands

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"sort"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

func TestCreate(t *testing.T) {
	tests := []struct {
		name        string
		project     string
		content     []byte
		expectError bool
		expectSave  bool
	}{
		{
			name:        "create with valid content",
			project:     "testproject",
			content:     []byte("Hello World\nThis is a test"),
			expectError: false,
			expectSave:  true,
		},
		{
			name:        "create with whitespace only",
			project:     "testproject",
			content:     []byte("   \n\t\n   "),
			expectError: false,
			expectSave:  false,
		},
		{
			name:        "create with leading/trailing whitespace",
			project:     "testproject",
			content:     []byte("\n\n  Hello World  \n\n"),
			expectError: false,
			expectSave:  true,
		},
		{
			name:        "create with single line",
			project:     "testproject",
			content:     []byte("Single line content"),
			expectError: false,
			expectSave:  true,
		},
		{
			name:        "create with multiline content",
			project:     "testproject",
			content:     []byte("Line 1\nLine 2\nLine 3\nLine 4"),
			expectError: false,
			expectSave:  true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			setup := SetupCommandTest(t)
			defer setup.Cleanup()

			initialCount := len(setup.Store.GetScratches())

			err := Create(setup.Store, tt.project, tt.content)
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
				}
			} else {
				if len(scratches) != initialCount {
					t.Errorf("expected scratch not to be saved, count: %d -> %d", initialCount, len(scratches))
				}
			}
		})
	}
}

func TestReadContentFromPipeWithReader(t *testing.T) {
	tests := []struct {
		name           string
		readerContent  string
		expectedResult []byte
	}{
		{
			name:           "read simple content",
			readerContent:  "Hello from reader",
			expectedResult: []byte("Hello from reader"),
		},
		{
			name:           "read multiline content",
			readerContent:  "Line 1\nLine 2\nLine 3",
			expectedResult: []byte("Line 1\nLine 2\nLine 3"),
		},
		{
			name:           "read empty content",
			readerContent:  "",
			expectedResult: []byte{},
		},
		{
			name:           "read content with special characters",
			readerContent:  "Content with \t tabs and \n newlines!",
			expectedResult: []byte("Content with \t tabs and \n newlines!"),
		},
		{
			name:           "read Unicode content",
			readerContent:  "Hello 世界 🌍",
			expectedResult: []byte("Hello 世界 🌍"),
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			reader := bytes.NewBufferString(tt.readerContent)
			result := ReadContentFromPipeWithReader(reader)

			if !bytes.Equal(result, tt.expectedResult) {
				t.Errorf("expected %q, got %q", tt.expectedResult, result)
			}
		})
	}
}

// TestReadContentFromPipeIntegration tests the integration between
// ReadContentFromPipe and the Create function
func TestReadContentFromPipeIntegration(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create a pipe with content
	r, w, err := os.Pipe()
	if err != nil {
		t.Fatal(err)
	}

	oldStdin := os.Stdin
	defer func() { os.Stdin = oldStdin }()
	os.Stdin = r

	// Write content to pipe
	testContent := "Content from pipe\nSecond line"
	go func() {
		defer func() { _ = w.Close() }()
		_, _ = fmt.Fprint(w, testContent)
	}()

	// Read content from pipe
	content := ReadContentFromPipe()
	if content == nil {
		t.Fatal("expected content from pipe, got nil")
	}

	// Create scratch with piped content
	err = Create(setup.Store, "testproject", content)
	if err != nil {
		t.Errorf("unexpected error creating scratch: %v", err)
	}

	// Verify scratch was created with correct content
	scratches := setup.Store.GetScratches()
	if len(scratches) != 1 {
		t.Fatalf("expected 1 scratch, got %d", len(scratches))
	}

	// Verify title is the first line
	if scratches[0].Title != "Content from pipe" {
		t.Errorf("expected title 'Content from pipe', got %q", scratches[0].Title)
	}

	// Verify content was saved correctly
	if scratches[0].Content != testContent {
		t.Errorf("expected saved content %q, got %q", testContent, scratches[0].Content)
	}
}

func TestGetTitle(t *testing.T) {
	tests := []struct {
		name          string
		content       []byte
		expectedTitle string
	}{
		{
			name:          "single line content",
			content:       []byte("This is the title"),
			expectedTitle: "This is the title",
		},
		{
			name:          "multiline content",
			content:       []byte("First Line Title\nSecond line content\nThird line"),
			expectedTitle: "First Line Title",
		},
		{
			name:          "empty content",
			content:       []byte(""),
			expectedTitle: "Untitled",
		},
		{
			name:          "content with leading newlines",
			content:       []byte("\n\nActual Title\nMore content"),
			expectedTitle: "",
		},
		{
			name:          "single word title",
			content:       []byte("Title"),
			expectedTitle: "Title",
		},
		{
			name:          "title with special characters",
			content:       []byte("Title with !@#$%^&*()"),
			expectedTitle: "Title with !@#$%^&*()",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			title := getTitle(tt.content)
			if title != tt.expectedTitle {
				t.Errorf("expected title '%s', got '%s'", tt.expectedTitle, title)
			}
		})
	}
}

func TestTrim(t *testing.T) {
	tests := []struct {
		name     string
		content  []byte
		expected []byte
	}{
		{
			name:     "no trimming needed",
			content:  []byte("hello world"),
			expected: []byte("hello world"),
		},
		{
			name:     "trim leading/trailing spaces",
			content:  []byte("   hello world   "),
			expected: []byte("hello world"),
		},
		{
			name:     "trim leading/trailing newlines",
			content:  []byte("\n\nhello world\n\n"),
			expected: []byte("hello world"),
		},
		{
			name:     "trim tabs and spaces",
			content:  []byte("\t  hello world  \t"),
			expected: []byte("hello world"),
		},
		{
			name:     "preserve internal whitespace",
			content:  []byte("\nhello\n\nworld\n"),
			expected: []byte("hello\n\nworld"),
		},
		{
			name:     "empty after trimming",
			content:  []byte("\n\t  \n\t"),
			expected: []byte(""),
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := trim(tt.content)
			if !bytes.Equal(result, tt.expected) {
				t.Errorf("expected %q, got %q", string(tt.expected), string(result))
			}
		})
	}
}

func TestReadContentFromPipe(t *testing.T) {
	tests := []struct {
		name           string
		pipeContent    string
		expectedResult []byte
		setupFunc      func() (cleanup func())
	}{
		{
			name:           "read from pipe with content",
			pipeContent:    "Hello from pipe",
			expectedResult: []byte("Hello from pipe"),
			setupFunc: func() func() {
				// Create a pipe
				r, w, err := os.Pipe()
				if err != nil {
					t.Fatal(err)
				}

				// Save original stdin
				oldStdin := os.Stdin
				os.Stdin = r

				// Write test content to pipe in a goroutine
				go func() {
					defer func() { _ = w.Close() }()
					_, _ = fmt.Fprint(w, "Hello from pipe")
				}()

				return func() {
					os.Stdin = oldStdin
					_ = r.Close()
				}
			},
		},
		{
			name:           "read from pipe with multiline content",
			pipeContent:    "Line 1\nLine 2\nLine 3",
			expectedResult: []byte("Line 1\nLine 2\nLine 3"),
			setupFunc: func() func() {
				r, w, err := os.Pipe()
				if err != nil {
					t.Fatal(err)
				}

				oldStdin := os.Stdin
				os.Stdin = r

				go func() {
					defer func() { _ = w.Close() }()
					_, _ = fmt.Fprint(w, "Line 1\nLine 2\nLine 3")
				}()

				return func() {
					os.Stdin = oldStdin
					_ = r.Close()
				}
			},
		},
		{
			name:           "no pipe input (terminal)",
			pipeContent:    "",
			expectedResult: nil,
			setupFunc: func() func() {
				// Use a regular file to simulate terminal input
				tmpFile, err := os.CreateTemp("", "stdin-test")
				if err != nil {
					t.Fatal(err)
				}

				oldStdin := os.Stdin
				os.Stdin = tmpFile

				return func() {
					os.Stdin = oldStdin
					_ = tmpFile.Close()
					_ = os.Remove(tmpFile.Name())
				}
			},
		},
		{
			name:           "empty pipe",
			pipeContent:    "",
			expectedResult: []byte{},
			setupFunc: func() func() {
				r, w, err := os.Pipe()
				if err != nil {
					t.Fatal(err)
				}

				oldStdin := os.Stdin
				os.Stdin = r

				// Close immediately to send EOF
				_ = w.Close()

				return func() {
					os.Stdin = oldStdin
					_ = r.Close()
				}
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			cleanup := tt.setupFunc()
			defer cleanup()

			result := ReadContentFromPipe()

			if !bytes.Equal(result, tt.expectedResult) {
				t.Errorf("expected %q, got %q", tt.expectedResult, result)
			}
		})
	}
}

func TestLs(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	now := time.Now()
	testScratches := []store.Scratch{
		{ID: "1", Project: "project1", Title: "Title 1", CreatedAt: now.Add(-3 * time.Hour)}, // oldest
		{ID: "2", Project: "project2", Title: "Title 2", CreatedAt: now.Add(-2 * time.Hour)},
		{ID: "3", Project: "global", Title: "Global Title", CreatedAt: now.Add(-1 * time.Hour)},
		{ID: "4", Project: "project1", Title: "Another P1", CreatedAt: now}, // newest
	}

	for _, scratch := range testScratches {
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}
	}

	tests := []struct {
		name          string
		global        bool
		project       string
		expectedCount int
		expectedIDs   []string
	}{
		{
			name:          "list project1 scratches - reverse chronological order",
			global:        false,
			project:       "project1",
			expectedCount: 2,
			expectedIDs:   []string{"4", "1"}, // newest first
		},
		{
			name:          "list project2 scratches",
			global:        false,
			project:       "project2",
			expectedCount: 1,
			expectedIDs:   []string{"2"},
		},
		{
			name:          "list global scratches",
			global:        true,
			project:       "",
			expectedCount: 1,
			expectedIDs:   []string{"3"},
		},
		{
			name:          "list non-existent project",
			global:        false,
			project:       "nonexistent",
			expectedCount: 0,
			expectedIDs:   []string{},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := Ls(setup.Store, tt.global, tt.project)
			if len(result) != tt.expectedCount {
				t.Errorf("expected %d scratches, got %d", tt.expectedCount, len(result))
			}

			resultIDs := make([]string, len(result))
			for i, scratch := range result {
				resultIDs[i] = scratch.ID
			}

			if !equalStringSlices(resultIDs, tt.expectedIDs) {
				t.Errorf("expected IDs %v, got %v", tt.expectedIDs, resultIDs)
			}
		})
	}
}

func TestSearch(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	now := time.Now()
	testScratches := []store.Scratch{
		{ID: "1", Project: "project1", Title: "Title 1", CreatedAt: now.Add(-2 * time.Hour)}, // older
		{ID: "2", Project: "project2", Title: "Title 2", CreatedAt: now.Add(-1 * time.Hour)}, // newer
		{ID: "3", Project: "global", Title: "Global Title", CreatedAt: now},                  // newest
	}

	testContents := map[string]string{
		"1": "This is content for scratch 1\nWith some test data",
		"2": "Different content for scratch 2\nNo such keyword here",
		"3": "Global scratch content\nSearching global data",
	}

	for _, scratch := range testScratches {
		scratch.Content = testContents[scratch.ID]
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}
	}

	tests := []struct {
		name          string
		global        bool
		project       string
		term          string
		expectedCount int
		expectedIDs   []string
		expectError   bool
	}{
		{
			name:          "search project1 for 'test'",
			global:        false,
			project:       "project1",
			term:          "test",
			expectedCount: 1,
			expectedIDs:   []string{"1"},
			expectError:   false,
		},
		{
			name:          "search project1 for 'content'",
			global:        false,
			project:       "project1",
			term:          "content",
			expectedCount: 1,
			expectedIDs:   []string{"1"},
			expectError:   false,
		},
		{
			name:          "search project2 for 'Different'",
			global:        false,
			project:       "project2",
			term:          "Different",
			expectedCount: 1,
			expectedIDs:   []string{"2"},
			expectError:   false,
		},
		{
			name:          "search for non-existent term in project1",
			global:        false,
			project:       "project1",
			term:          "nonexistent",
			expectedCount: 0,
			expectedIDs:   []string{},
			expectError:   false,
		},
		{
			name:          "invalid regex",
			global:        false,
			project:       "project1",
			term:          "[invalid",
			expectedCount: 0,
			expectedIDs:   []string{},
			expectError:   true,
		},
		{
			name:          "case sensitive search in project1",
			global:        false,
			project:       "project1",
			term:          "Test",
			expectedCount: 0,
			expectedIDs:   []string{},
			expectError:   false,
		},
		{
			name:          "search global for 'global'",
			global:        true,
			project:       "",
			term:          "global",
			expectedCount: 1,
			expectedIDs:   []string{"3"},
			expectError:   false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := Search(setup.Store, tt.global, tt.project, tt.term)

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

			if len(result) != tt.expectedCount {
				t.Errorf("expected %d results, got %d", tt.expectedCount, len(result))
			}

			resultIDs := make([]string, len(result))
			for i, scratch := range result {
				resultIDs[i] = scratch.ID
			}

			if !equalStringSlices(resultIDs, tt.expectedIDs) {
				t.Errorf("expected IDs %v, got %v", tt.expectedIDs, resultIDs)
			}
		})
	}
}

func TestSearchWithIndices(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	now := time.Now()
	testScratches := []store.Scratch{
		{ID: "1", Project: "p1", Title: "A", CreatedAt: now.Add(-4 * time.Hour)}, // Will be index 4
		{ID: "2", Project: "p2", Title: "B", CreatedAt: now.Add(-3 * time.Hour)}, // Will be index 3
		{ID: "3", Project: "p1", Title: "C", CreatedAt: now.Add(-2 * time.Hour)}, // Will be index 2
		{ID: "4", Project: "p2", Title: "D", CreatedAt: now.Add(-1 * time.Hour)}, // Will be index 1
	}

	testContents := map[string]string{
		"1": "Content for one",
		"2": "Content for two",
		"3": "Content for three",
		"4": "Content for four",
	}

	for _, scratch := range testScratches {
		scratch.Content = testContents[scratch.ID]
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}
	}

	tests := []struct {
		name             string
		global           bool
		project          string
		term             string
		expectedCount    int
		expectedIndices  []string // Now using nanostore SimpleIDs as strings
		expectedOrderIDs []string
		expectError      bool
	}{
		{
			name:             "search p1, find one with 'three'",
			project:          "p1",
			term:             "three",
			expectedCount:    1,
			expectedIndices:  []string{"3"}, // Actual nanostore SimpleID
			expectedOrderIDs: []string{"3"},
		},
		{
			name:             "search p2, find one with 'two'",
			project:          "p2",
			term:             "two",
			expectedCount:    1,
			expectedIndices:  []string{"2"}, // Actual nanostore SimpleID
			expectedOrderIDs: []string{"2"},
		},
		{
			name:             "search p1, no results",
			project:          "p1",
			term:             "nonexistent",
			expectedCount:    0,
			expectedIndices:  []string{},
			expectedOrderIDs: []string{},
		},
		{
			name:          "invalid regex",
			project:       "p1",
			term:          "[invalid",
			expectError:   true,
			expectedCount: 0,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			results, err := SearchWithIndices(setup.Store, tt.global, tt.project, tt.term)

			if tt.expectError {
				if err == nil {
					t.Fatal("expected error but got none")
				}
				return
			}
			if err != nil {
				t.Fatalf("unexpected error: %v", err)
			}

			if len(results) != tt.expectedCount {
				t.Fatalf("expected %d results, got %d", tt.expectedCount, len(results))
			}

			if tt.expectedCount > 0 {
				indices := make([]string, len(results))
				ids := make([]string, len(results))
				for i, r := range results {
					indices[i] = r.Index
					ids[i] = r.ID
				}

				// Check indices (now nanostore SimpleIDs)
				if !equalStringSlices(indices, tt.expectedIndices) {
					t.Errorf("expected indices %v, got %v", tt.expectedIndices, indices)
				}

				// Check order
				if !equalStringSlices(ids, tt.expectedOrderIDs) {
					t.Errorf("expected order %v, got %v", tt.expectedOrderIDs, ids)
				}
			}
		})
	}
}

func TestView(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	testContent := "Line 1\nLine 2\nLine 3"
	testScratch := store.Scratch{
		ID:        "test1",
		Project:   "testproject",
		Title:     "Test Title",
		Content:   testContent,
		CreatedAt: time.Now(),
	}
	if err := setup.Store.AddScratch(testScratch); err != nil {
		t.Fatalf("failed to add test scratch: %v", err)
	}

	tests := []struct {
		name            string
		all             bool
		global          bool
		project         string
		indexStr        string
		expectedContent string
		expectError     bool
	}{
		{
			name:            "view valid scratch",
			all:             false,
			global:          false,
			project:         "testproject",
			indexStr:        "1",
			expectedContent: testContent,
			expectError:     false,
		},
		{
			name:            "invalid index string",
			all:             false,
			global:          false,
			project:         "testproject",
			indexStr:        "invalid",
			expectedContent: "",
			expectError:     true,
		},
		{
			name:            "index out of range - too high",
			all:             false,
			global:          false,
			project:         "testproject",
			indexStr:        "99",
			expectedContent: "",
			expectError:     true,
		},
		{
			name:            "index out of range - zero",
			all:             false,
			global:          false,
			project:         "testproject",
			indexStr:        "0",
			expectedContent: "",
			expectError:     true,
		},
		{
			name:            "index out of range - negative",
			all:             false,
			global:          false,
			project:         "testproject",
			indexStr:        "-1",
			expectedContent: "",
			expectError:     true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := View(setup.Store, tt.global, tt.project, tt.indexStr)

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

			if result != tt.expectedContent {
				t.Errorf("expected content %q, got %q", tt.expectedContent, result)
			}
		})
	}
}

func TestPeek(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create test scratches in different projects with distinct timestamps
	now := time.Now()
	testScratches := []struct {
		scratch store.Scratch
		content string
	}{
		{
			scratch: store.Scratch{ID: "test1", Project: "testproject", Title: "Test Title", CreatedAt: now.Add(-2 * time.Hour)}, // older
			content: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8",
		},
		{
			scratch: store.Scratch{ID: "test2", Project: "otherproject", Title: "Other Title", CreatedAt: now.Add(-1 * time.Hour)}, // middle
			content: "Other Line 1\nOther Line 2\nOther Line 3\nOther Line 4\nOther Line 5",
		},
		{
			scratch: store.Scratch{ID: "test3", Project: "global", Title: "Global Title", CreatedAt: now}, // newest
			content: "Global Line 1\nGlobal Line 2\nGlobal Line 3",
		},
	}

	for _, ts := range testScratches {
		ts.scratch.Content = ts.content
		if err := setup.Store.AddScratch(ts.scratch); err != nil {
			t.Fatalf("failed to add test scratch: %v", err)
		}
	}

	tests := []struct {
		name            string
		project         string
		index           string
		lines           int
		global          bool
		expectedContent string
		expectError     bool
	}{
		{
			name:            "short content - return all",
			project:         "global",
			index:           "3", // Third scratch added has SimpleID=3
			lines:           3,
			expectedContent: "Global Line 1\nGlobal Line 2\nGlobal Line 3",
		},
		{
			name:            "long content - peek with ellipsis",
			project:         "testproject",
			index:           "1",
			lines:           2,
			expectedContent: "Line 1\nLine 2\n...\nLine 7\nLine 8\n",
		},
		{
			name:            "default 3 lines peek",
			project:         "testproject",
			index:           "1",
			lines:           3,
			expectedContent: "Line 1\nLine 2\nLine 3\n...\nLine 6\nLine 7\nLine 8\n",
		},
		{
			name:            "exactly 2*lines content",
			project:         "otherproject",
			index:           "2", // Second scratch added has SimpleID=2
			lines:           2,
			expectedContent: "Other Line 1\nOther Line 2\n...\nOther Line 4\nOther Line 5\n",
		},
		{
			name:        "invalid index",
			project:     "testproject",
			index:       "99",
			lines:       3,
			expectError: true,
		},
		{
			name:            "peek second scratch in otherproject",
			project:         "otherproject",
			index:           "2", // Second scratch added has SimpleID=2
			lines:           2,
			expectedContent: "Other Line 1\nOther Line 2\n...\nOther Line 4\nOther Line 5\n",
		},
		{
			name:            "peek with global flag",
			project:         "",
			index:           "3", // Third scratch added has SimpleID=3
			lines:           2,
			global:          true,
			expectedContent: "Global Line 1\nGlobal Line 2\nGlobal Line 3",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := Peek(setup.Store, tt.global, tt.project, tt.index, tt.lines)
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

			if result != tt.expectedContent {
				t.Errorf("expected content:\n%q\ngot:\n%q", tt.expectedContent, result)
			}
		})
	}
}

func TestDelete(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	testScratch := store.Scratch{
		ID:        "test1",
		Project:   "testproject",
		Title:     "Test Title",
		Content:   "test content",
		CreatedAt: time.Now(),
	}
	if err := setup.Store.AddScratch(testScratch); err != nil {
		t.Fatalf("failed to add test scratch: %v", err)
	}

	tests := []struct {
		name        string
		project     string
		indexStr    string
		expectError bool
	}{
		{
			name:        "delete valid scratch",
			project:     "testproject",
			indexStr:    "1",
			expectError: false,
		},
		{
			name:        "invalid index",
			project:     "testproject",
			indexStr:    "invalid",
			expectError: true,
		},
		{
			name:        "index out of range",
			project:     "testproject",
			indexStr:    "99",
			expectError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			initialCount := len(setup.Store.GetScratches())

			err := Delete(setup.Store, false, tt.project, tt.indexStr)

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

			// With soft delete, active count should decrease
			if len(setup.Store.GetScratches()) != initialCount-1 {
				t.Errorf("expected scratch count to decrease by 1 with soft delete")
			}

			// Verify scratch is marked as deleted in all scratches
			allScratches := setup.Store.GetAllScratches()
			found := false
			for _, s := range allScratches {
				if s.Title == testScratch.Title {
					found = true
					if !s.IsDeleted {
						t.Errorf("expected scratch to be marked as deleted")
					}
					if s.DeletedAt == nil {
						t.Errorf("expected DeletedAt to be set")
					}
					break
				}
			}
			if !found {
				t.Errorf("expected to find the soft-deleted scratch")
			}
		})
	}
}

func TestCleanup(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Get test store for setting custom timestamps
	testStore, ok := setup.Store.GetTestStore()
	if !ok {
		t.Skip("Test store not available")
	}

	now := time.Now()
	testScratches := []struct {
		scratch   store.Scratch
		createdAt time.Time
	}{
		{store.Scratch{ID: "old1", Project: "test", Title: "Old 1", Content: "content"}, now.AddDate(0, 0, -10)},
		{store.Scratch{ID: "old2", Project: "test", Title: "Old 2", Content: "content"}, now.AddDate(0, 0, -8)},
		{store.Scratch{ID: "new1", Project: "test", Title: "New 1", Content: "content"}, now.AddDate(0, 0, -5)},
		{store.Scratch{ID: "new2", Project: "test", Title: "New 2", Content: "content"}, now.Add(-12 * time.Hour)}, // Less than 1 day
	}

	// Add each scratch with its specific timestamp
	for _, ts := range testScratches {
		// Set the time function to return the specific timestamp
		testStore.SetTimeFunc(func() time.Time { return ts.createdAt })

		if err := setup.Store.AddScratch(ts.scratch); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}
	}

	// Reset time function to normal
	testStore.SetTimeFunc(time.Now)

	tests := []struct {
		name                    string
		days                    int
		expectedRemaining       int
		expectedRemainingTitles []string
	}{
		{
			name:                    "cleanup 7 days",
			days:                    7,
			expectedRemaining:       2,
			expectedRemainingTitles: []string{"New 1", "New 2"},
		},
		{
			name:                    "cleanup 1 day",
			days:                    1,
			expectedRemaining:       1,
			expectedRemainingTitles: []string{"New 2"},
		},
		{
			name:                    "cleanup 20 days",
			days:                    20,
			expectedRemaining:       4,
			expectedRemainingTitles: []string{"Old 1", "Old 2", "New 1", "New 2"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Create a new test environment for each subtest
			subSetup := SetupCommandTest(t)
			defer subSetup.Cleanup()

			// Get test store for setting custom timestamps
			subTestStore, ok := subSetup.Store.GetTestStore()
			if !ok {
				t.Skip("Test store not available")
			}

			// Add each scratch with its specific timestamp
			for _, ts := range testScratches {
				// Set the time function to return the specific timestamp
				subTestStore.SetTimeFunc(func() time.Time { return ts.createdAt })

				if err := subSetup.Store.AddScratch(ts.scratch); err != nil {
					t.Fatalf("failed to add scratch: %v", err)
				}
			}

			// Reset time function to normal
			subTestStore.SetTimeFunc(time.Now)

			err := Cleanup(subSetup.Store, tt.days)
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			remaining := subSetup.Store.GetScratches()
			if len(remaining) != tt.expectedRemaining {
				t.Errorf("expected %d remaining scratches, got %d", tt.expectedRemaining, len(remaining))
			}

			remainingTitles := make([]string, len(remaining))
			for i, scratch := range remaining {
				remainingTitles[i] = scratch.Title
			}
			sort.Strings(remainingTitles)
			sort.Strings(tt.expectedRemainingTitles)

			if !equalStringSlices(remainingTitles, tt.expectedRemainingTitles) {
				t.Errorf("expected remaining titles %v, got %v", tt.expectedRemainingTitles, remainingTitles)
			}
		})
	}
}

func TestOpen(t *testing.T) {
	// Skip if ed is not available
	if _, err := exec.LookPath("ed"); err != nil {
		t.Skip("ed editor not available in PATH")
	}

	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create a test scratch
	originalContent := []byte("Original content\nLine 2\nLine 3")
	err := Create(setup.Store, "testproject", originalContent)
	if err != nil {
		t.Fatalf("failed to create scratch: %v", err)
	}

	// Set up ed as the editor with a script that modifies the file
	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	// Create an ed script that will append a line
	edScript := createEdScript(t, []string{
		"$a", // append after last line
		"New line added by ed",
		".", // end append mode
		"w", // write file
		"q", // quit
	})
	defer func() { _ = os.Remove(edScript) }()

	// Set EDITOR to run ed with our script
	editorCmd := fmt.Sprintf("ed -s < %s", edScript)
	wrapperScript := createEditorWrapper(t, editorCmd)
	defer func() { _ = os.Remove(wrapperScript) }()
	if err := os.Setenv("EDITOR", wrapperScript); err != nil {
		t.Fatalf("failed to set EDITOR: %v", err)
	}

	// Test opening and editing the scratch
	err = Open(setup.Store, false, "testproject", "1")
	if err != nil {
		t.Errorf("unexpected error opening scratch: %v", err)
	}

	// Verify the content was modified
	scratches := setup.Store.GetScratches()
	if len(scratches) != 1 {
		t.Fatalf("expected 1 scratch, got %d", len(scratches))
	}

	expectedContent := "Original content\nLine 2\nLine 3\nNew line added by ed"
	if scratches[0].Content != expectedContent {
		t.Errorf("content mismatch:\nexpected: %q\ngot: %q", expectedContent, scratches[0].Content)
	}

	// Test that title is updated
	if scratches[0].Title != "Original content" {
		t.Errorf("expected title 'Original content', got %q", scratches[0].Title)
	}
}

func TestOpen_DeletesEmptyScratch(t *testing.T) {
	// Skip if ed is not available
	if _, err := exec.LookPath("ed"); err != nil {
		t.Skip("ed editor not available in PATH")
	}

	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Create a test scratch
	err := Create(setup.Store, "testproject", []byte("Content to be deleted"))
	if err != nil {
		t.Fatalf("failed to create scratch: %v", err)
	}

	// Set up ed to delete all content
	oldEditor := os.Getenv("EDITOR")
	defer func() { _ = os.Setenv("EDITOR", oldEditor) }()

	edScript := createEdScript(t, []string{
		"1,$d", // delete all lines
		"w",    // write file
		"q",    // quit
	})
	defer func() { _ = os.Remove(edScript) }()

	editorCmd := fmt.Sprintf("ed -s < %s", edScript)
	wrapperScript := createEditorWrapper(t, editorCmd)
	defer func() { _ = os.Remove(wrapperScript) }()
	if err := os.Setenv("EDITOR", wrapperScript); err != nil {
		t.Fatalf("failed to set EDITOR: %v", err)
	}

	// Open and edit (delete all content)
	err = Open(setup.Store, false, "testproject", "1")
	if err != nil {
		t.Errorf("unexpected error opening scratch: %v", err)
	}

	// Verify the scratch was soft-deleted
	activeScratches := setup.Store.GetScratches()
	if len(activeScratches) != 0 {
		t.Errorf("expected 0 active scratches (should be soft-deleted), but found %d", len(activeScratches))
		return
	}

	// Check deleted scratches
	deletedScratches := setup.Store.GetDeletedScratches()
	if len(deletedScratches) != 1 {
		t.Errorf("expected 1 deleted scratch, but found %d", len(deletedScratches))
		return
	}

	scratch := deletedScratches[0]
	if !scratch.IsDeleted {
		t.Errorf("expected scratch to be marked as deleted")
	}
	if scratch.DeletedAt == nil {
		t.Errorf("expected DeletedAt to be set")
	}
}

// createEdScript creates a temporary file with ed commands
func createEdScript(t *testing.T, commands []string) string {
	tmpFile, err := os.CreateTemp("", "ed-script-*.txt")
	if err != nil {
		t.Fatalf("failed to create ed script: %v", err)
	}

	for _, cmd := range commands {
		if _, err := fmt.Fprintln(tmpFile, cmd); err != nil {
			t.Fatalf("failed to write ed command: %v", err)
		}
	}

	if err := tmpFile.Close(); err != nil {
		t.Fatalf("failed to close ed script: %v", err)
	}

	return tmpFile.Name()
}

// createEditorWrapper creates a shell script that runs ed with redirection
func createEditorWrapper(t *testing.T, editorCmd string) string {
	script := fmt.Sprintf(`#!/bin/sh
# Editor wrapper for testing
FILE="$1"
%s "$FILE"
`, editorCmd)

	tmpFile, err := os.CreateTemp("", "editor-wrapper-*.sh")
	if err != nil {
		t.Fatalf("failed to create editor wrapper: %v", err)
	}

	if _, err := tmpFile.WriteString(script); err != nil {
		t.Fatalf("failed to write editor wrapper: %v", err)
	}

	if err := tmpFile.Close(); err != nil {
		t.Fatalf("failed to close editor wrapper: %v", err)
	}

	if err := os.Chmod(tmpFile.Name(), 0755); err != nil {
		t.Fatalf("failed to make editor wrapper executable: %v", err)
	}

	return tmpFile.Name()
}

// TestSortByCreatedAtDesc has been removed since nanostore now handles sorting
// at the database level using OrderBy in ListOptions

func TestGetScratchByIndex(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	now := time.Now()
	testScratches := []store.Scratch{
		{ID: "1", Project: "project1", Title: "Title 1", CreatedAt: now.Add(-3 * time.Hour)},    // oldest, will be index 3
		{ID: "2", Project: "project2", Title: "Title 2", CreatedAt: now.Add(-2 * time.Hour)},    // index 3 in all
		{ID: "3", Project: "global", Title: "Global Title", CreatedAt: now.Add(-1 * time.Hour)}, // index 2 in all
		{ID: "4", Project: "project1", Title: "Another P1", CreatedAt: now},                     // newest, will be index 1
	}

	for _, scratch := range testScratches {
		if err := setup.Store.AddScratch(scratch); err != nil {
			t.Fatalf("failed to add scratch: %v", err)
		}
	}

	tests := []struct {
		name        string
		global      bool
		project     string
		indexStr    string
		expectedID  string
		expectError bool
	}{
		{
			name:        "get scratch with SimpleID 1 in project1",
			global:      false,
			project:     "project1",
			indexStr:    "1",
			expectedID:  "1", // SimpleID 1 resolves to itself if it exists in scope
			expectError: false,
		},
		{
			name:        "get scratch with SimpleID 4 in project1",
			global:      false,
			project:     "project1",
			indexStr:    "4",
			expectedID:  "4", // SimpleID 4 resolves to itself if it exists in scope
			expectError: false,
		},
		{
			name:        "get scratch with SimpleID 2 in project2",
			global:      false,
			project:     "project2",
			indexStr:    "2",
			expectedID:  "2", // SimpleID 2 should exist in project2
			expectError: false,
		},
		{
			name:        "get scratch with SimpleID 3 in global scope",
			global:      true,
			project:     "",
			indexStr:    "3",
			expectedID:  "3", // SimpleID 3 should exist in global scope
			expectError: false,
		},
		{
			name:        "invalid index string",
			global:      false,
			project:     "project1",
			indexStr:    "invalid",
			expectedID:  "",
			expectError: true,
		},
		{
			name:        "index out of range - too high",
			global:      false,
			project:     "project1",
			indexStr:    "99",
			expectedID:  "",
			expectError: true,
		},
		{
			name:        "index out of range - zero",
			global:      false,
			project:     "project1",
			indexStr:    "0",
			expectedID:  "",
			expectError: true,
		},
		{
			name:        "index out of range - negative",
			global:      false,
			project:     "project1",
			indexStr:    "-1",
			expectedID:  "",
			expectError: true,
		},
		{
			name:        "no scratches in non-existent project",
			global:      false,
			project:     "nonexistent",
			indexStr:    "1",
			expectedID:  "",
			expectError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := GetScratchByIndex(setup.Store, tt.global, tt.project, tt.indexStr)

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

			if result.ID != tt.expectedID {
				t.Errorf("expected ID %s, got %s", tt.expectedID, result.ID)
			}
		})
	}
}

func TestCreateWithEmptyContent(t *testing.T) {
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	// Set editor to a command that will write test content and exit
	oldEditor := os.Getenv("EDITOR")
	testScript := createMockEditorScript(t, "test content from editor")
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

	// Test with empty content - should launch editor
	err := Create(setup.Store, "testproject", []byte(""))
	if err != nil {
		t.Errorf("unexpected error: %v", err)
		return
	}

	scratches := setup.Store.GetScratches()
	if len(scratches) != initialCount+1 {
		t.Errorf("expected scratch to be saved via editor, count: %d -> %d", initialCount, len(scratches))
	}
}

func createMockEditorScript(t *testing.T, content string) string {
	scriptContent := `#!/bin/bash
echo "` + content + `" > "$1"
`
	tmpFile, err := os.CreateTemp("", "mockeditor-*.sh")
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

func equalStringSlices(a, b []string) bool {
	if len(a) != len(b) {
		return false
	}
	for i, v := range a {
		if v != b[i] {
			return false
		}
	}
	return true
}
