package commands

import (
	"bytes"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"

	"github.com/adrg/xdg"
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
			tmpDir := setupCommandsTestDir(t)
			defer os.RemoveAll(tmpDir)

			s, err := store.NewStore()
			if err != nil {
				t.Fatalf("failed to create store: %v", err)
			}

			initialCount := len(s.GetScratches())

			err = Create(s, tt.project, tt.content)
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

			scratches := s.GetScratches()
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
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

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
		defer w.Close()
		fmt.Fprint(w, testContent)
	}()

	// Read content from pipe
	content := ReadContentFromPipe()
	if content == nil {
		t.Fatal("expected content from pipe, got nil")
	}

	// Create scratch with piped content
	err = Create(s, "testproject", content)
	if err != nil {
		t.Errorf("unexpected error creating scratch: %v", err)
	}

	// Verify scratch was created with correct content
	scratches := s.GetScratches()
	if len(scratches) != 1 {
		t.Fatalf("expected 1 scratch, got %d", len(scratches))
	}

	// Verify title is the first line
	if scratches[0].Title != "Content from pipe" {
		t.Errorf("expected title 'Content from pipe', got %q", scratches[0].Title)
	}

	// Verify content was saved correctly
	savedContent, err := readScratchFile(scratches[0].ID)
	if err != nil {
		t.Fatalf("failed to read scratch file: %v", err)
	}

	if string(savedContent) != testContent {
		t.Errorf("expected saved content %q, got %q", testContent, string(savedContent))
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
				defer w.Close()
				fmt.Fprint(w, "Hello from pipe")
			}()

			return func() {
				os.Stdin = oldStdin
				r.Close()
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
				defer w.Close()
				fmt.Fprint(w, "Line 1\nLine 2\nLine 3")
			}()

			return func() {
				os.Stdin = oldStdin
				r.Close()
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
				tmpFile.Close()
				os.Remove(tmpFile.Name())
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
			w.Close()

			return func() {
				os.Stdin = oldStdin
				r.Close()
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
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	now := time.Now()
	testScratches := []store.Scratch{
		{ID: "1", Project: "project1", Title: "Title 1", CreatedAt: now.Add(-3 * time.Hour)}, // oldest
		{ID: "2", Project: "project2", Title: "Title 2", CreatedAt: now.Add(-2 * time.Hour)},
		{ID: "3", Project: "global", Title: "Global Title", CreatedAt: now.Add(-1 * time.Hour)},
		{ID: "4", Project: "project1", Title: "Another P1", CreatedAt: now}, // newest
	}

	for _, scratch := range testScratches {
		s.AddScratch(scratch)
	}

	tests := []struct {
		name          string
		all           bool
		global        bool
		project       string
		expectedCount int
		expectedIDs   []string
	}{
		{
			name:          "list all scratches - reverse chronological order",
			all:           true,
			global:        false,
			project:       "",
			expectedCount: 4,
			expectedIDs:   []string{"4", "3", "2", "1"}, // newest first
		},
		{
			name:          "list project1 scratches - reverse chronological order",
			all:           false,
			global:        false,
			project:       "project1",
			expectedCount: 2,
			expectedIDs:   []string{"4", "1"}, // newest first
		},
		{
			name:          "list project2 scratches",
			all:           false,
			global:        false,
			project:       "project2",
			expectedCount: 1,
			expectedIDs:   []string{"2"},
		},
		{
			name:          "list global scratches",
			all:           false,
			global:        true,
			project:       "",
			expectedCount: 1,
			expectedIDs:   []string{"3"},
		},
		{
			name:          "list non-existent project",
			all:           false,
			global:        false,
			project:       "nonexistent",
			expectedCount: 0,
			expectedIDs:   []string{},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := Ls(s, tt.all, tt.global, tt.project)
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
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	now := time.Now()
	testScratches := []store.Scratch{
		{ID: "1", Project: "project1", Title: "Title 1", CreatedAt: now.Add(-2 * time.Hour)}, // older
		{ID: "2", Project: "project2", Title: "Title 2", CreatedAt: now.Add(-1 * time.Hour)}, // newer
		{ID: "3", Project: "global", Title: "Global Title", CreatedAt: now}, // newest
	}

	testContents := map[string]string{
		"1": "This is content for scratch 1\nWith some test data",
		"2": "Different content for scratch 2\nNo such keyword here",
		"3": "Global scratch content\nSearching global data",
	}

	for _, scratch := range testScratches {
		s.AddScratch(scratch)
		saveScratchFile(scratch.ID, []byte(testContents[scratch.ID]))
	}

	tests := []struct {
		name          string
		all           bool
		global        bool
		project       string
		term          string
		expectedCount int
		expectedIDs   []string
		expectError   bool
	}{
		{
			name:          "search all for 'test'",
			all:           true,
			global:        false,
			project:       "",
			term:          "test",
			expectedCount: 1,
			expectedIDs:   []string{"1"},
			expectError:   false,
		},
		{
			name:          "search project1 for 'content'", 
			all:           false,
			global:        false,
			project:       "project1",
			term:          "content",
			expectedCount: 1,
			expectedIDs:   []string{"1"},
			expectError:   false,
		},
		{
			name:          "search for non-existent term",
			all:           true,
			global:        false,
			project:       "",
			term:          "nonexistent",
			expectedCount: 0,
			expectedIDs:   []string{},
			expectError:   false,
		},
		{
			name:          "invalid regex",
			all:           true,
			global:        false,
			project:       "",
			term:          "[invalid",
			expectedCount: 0,
			expectedIDs:   []string{},
			expectError:   true,
		},
		{
			name:          "case sensitive search",
			all:           true,
			global:        false,
			project:       "",
			term:          "Test",
			expectedCount: 0,
			expectedIDs:   []string{},
			expectError:   false,
		},
		{
			name:          "regex pattern search - reverse chronological order",
			all:           true,
			global:        false,
			project:       "",
			term:          "scratch [0-9]",
			expectedCount: 2,
			expectedIDs:   []string{"2", "1"}, // newest first (2 is newer than 1)
			expectError:   false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := Search(s, tt.all, tt.global, tt.project, tt.term)
			
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

func TestView(t *testing.T) {
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	testScratch := store.Scratch{
		ID: "test1", Project: "testproject", Title: "Test Title", CreatedAt: time.Now(),
	}
	s.AddScratch(testScratch)

	testContent := "Line 1\nLine 2\nLine 3"
	saveScratchFile(testScratch.ID, []byte(testContent))

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
			result, err := View(s, tt.all, tt.global, tt.project, tt.indexStr)
			
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
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

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
		s.AddScratch(ts.scratch)
		if err := saveScratchFile(ts.scratch.ID, []byte(ts.content)); err != nil {
			t.Fatalf("failed to save scratch file: %v", err)
		}
	}

	tests := []struct {
		name            string
		project         string
		index           string
		lines           int
		all             bool
		global          bool
		expectedContent string
		expectError     bool
	}{
		{
			name:            "short content - return all",
			project:         "global",
			index:           "1",
			lines:           3,
			expectedContent: "Global Line 1\nGlobal Line 2\nGlobal Line 3",
		},
		{
			name:    "long content - peek with ellipsis",
			project: "testproject",
			index:   "1",
			lines:   2,
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
			index:           "1",
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
			name:            "peek with all flag",
			project:         "",
			index:           "2",
			lines:           2,
			all:             true,
			expectedContent: "Other Line 1\nOther Line 2\n...\nOther Line 4\nOther Line 5\n",
		},
		{
			name:            "peek with global flag",
			project:         "",
			index:           "1",
			lines:           2,
			global:          true,
			expectedContent: "Global Line 1\nGlobal Line 2\nGlobal Line 3",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := Peek(s, tt.all, tt.global, tt.project, tt.index, tt.lines)
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
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	testScratch := store.Scratch{
		ID: "test1", Project: "testproject", Title: "Test Title", CreatedAt: time.Now(),
	}
	s.AddScratch(testScratch)
	saveScratchFile(testScratch.ID, []byte("test content"))

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
			initialCount := len(s.GetScratches())

			err := Delete(s, false, tt.project, tt.indexStr)
			
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

			if len(s.GetScratches()) != initialCount-1 {
				t.Errorf("expected scratch count to decrease by 1")
			}

			path, _ := store.GetScratchFilePath(testScratch.ID)
			if _, err := os.Stat(path); !os.IsNotExist(err) {
				t.Errorf("expected scratch file to be deleted")
			}
		})
	}
}

func TestCleanup(t *testing.T) {
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	now := time.Now()
	testScratches := []store.Scratch{
		{ID: "old1", Project: "test", Title: "Old 1", CreatedAt: now.AddDate(0, 0, -10)},
		{ID: "old2", Project: "test", Title: "Old 2", CreatedAt: now.AddDate(0, 0, -8)},
		{ID: "new1", Project: "test", Title: "New 1", CreatedAt: now.AddDate(0, 0, -5)},
		{ID: "new2", Project: "test", Title: "New 2", CreatedAt: now.Add(-12 * time.Hour)}, // Less than 1 day
	}

	for _, scratch := range testScratches {
		s.AddScratch(scratch)
		saveScratchFile(scratch.ID, []byte("content"))
	}

	tests := []struct {
		name                string
		days                int
		expectedRemaining   int
		expectedRemainingIDs []string
	}{
		{
			name:                "cleanup 7 days",
			days:                7,
			expectedRemaining:   2,
			expectedRemainingIDs: []string{"new1", "new2"},
		},
		{
			name:                "cleanup 1 day",
			days:                1,
			expectedRemaining:   1,
			expectedRemainingIDs: []string{"new2"},
		},
		{
			name:                "cleanup 20 days",
			days:                20,
			expectedRemaining:   4,
			expectedRemainingIDs: []string{"old1", "old2", "new1", "new2"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Reset store for each test
			s.SaveScratches(testScratches)
			for _, scratch := range testScratches {
				saveScratchFile(scratch.ID, []byte("content"))
			}

			err := Cleanup(s, tt.days)
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			remaining := s.GetScratches()
			if len(remaining) != tt.expectedRemaining {
				t.Errorf("expected %d remaining scratches, got %d", tt.expectedRemaining, len(remaining))
			}

			remainingIDs := make([]string, len(remaining))
			for i, scratch := range remaining {
				remainingIDs[i] = scratch.ID
			}

			if !equalStringSlices(remainingIDs, tt.expectedRemainingIDs) {
				t.Errorf("expected remaining IDs %v, got %v", tt.expectedRemainingIDs, remainingIDs)
			}
		})
	}
}

func TestOpen(t *testing.T) {
	// Skip if ed is not available
	if _, err := exec.LookPath("ed"); err != nil {
		t.Skip("ed editor not available in PATH")
	}

	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	// Create a test scratch
	originalContent := []byte("Original content\nLine 2\nLine 3")
	err = Create(s, "testproject", originalContent)
	if err != nil {
		t.Fatalf("failed to create scratch: %v", err)
	}

	// Set up ed as the editor with a script that modifies the file
	oldEditor := os.Getenv("EDITOR")
	defer os.Setenv("EDITOR", oldEditor)

	// Create an ed script that will append a line
	edScript := createEdScript(t, []string{
		"$a",           // append after last line
		"New line added by ed",
		".",            // end append mode
		"w",            // write file
		"q",            // quit
	})
	defer os.Remove(edScript)

	// Set EDITOR to run ed with our script
	editorCmd := fmt.Sprintf("ed -s < %s", edScript)
	wrapperScript := createEditorWrapper(t, editorCmd)
	defer os.Remove(wrapperScript)
	os.Setenv("EDITOR", wrapperScript)

	// Test opening and editing the scratch
	err = Open(s, false, "testproject", "1")
	if err != nil {
		t.Errorf("unexpected error opening scratch: %v", err)
	}

	// Verify the content was modified
	scratches := s.GetScratches()
	if len(scratches) != 1 {
		t.Fatalf("expected 1 scratch, got %d", len(scratches))
	}

	content, err := readScratchFile(scratches[0].ID)
	if err != nil {
		t.Fatalf("failed to read scratch content: %v", err)
	}

	expectedContent := "Original content\nLine 2\nLine 3\nNew line added by ed"
	if string(content) != expectedContent {
		t.Errorf("content mismatch:\nexpected: %q\ngot: %q", expectedContent, string(content))
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

	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	// Create a test scratch
	err = Create(s, "testproject", []byte("Content to be deleted"))
	if err != nil {
		t.Fatalf("failed to create scratch: %v", err)
	}

	// Set up ed to delete all content
	oldEditor := os.Getenv("EDITOR")
	defer os.Setenv("EDITOR", oldEditor)

	edScript := createEdScript(t, []string{
		"1,$d",         // delete all lines
		"w",            // write file
		"q",            // quit
	})
	defer os.Remove(edScript)

	editorCmd := fmt.Sprintf("ed -s < %s", edScript)
	wrapperScript := createEditorWrapper(t, editorCmd)
	defer os.Remove(wrapperScript)
	os.Setenv("EDITOR", wrapperScript)

	// Open and edit (delete all content)
	err = Open(s, false, "testproject", "1")
	if err != nil {
		t.Errorf("unexpected error opening scratch: %v", err)
	}

	// Verify the scratch was deleted
	scratches := s.GetScratches()
	if len(scratches) != 0 {
		t.Errorf("expected scratch to be deleted, but found %d scratches", len(scratches))
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

func TestSortByCreatedAtDesc(t *testing.T) {
	now := time.Now()
	testScratches := []store.Scratch{
		{ID: "oldest", Project: "test", Title: "Oldest", CreatedAt: now.Add(-3 * time.Hour)},
		{ID: "newest", Project: "test", Title: "Newest", CreatedAt: now},
		{ID: "middle", Project: "test", Title: "Middle", CreatedAt: now.Add(-1 * time.Hour)},
		{ID: "old", Project: "test", Title: "Old", CreatedAt: now.Add(-2 * time.Hour)},
	}

	sorted := sortByCreatedAtDesc(testScratches)

	expected := []string{"newest", "middle", "old", "oldest"}
	actual := make([]string, len(sorted))
	for i, scratch := range sorted {
		actual[i] = scratch.ID
	}

	if !equalStringSlices(actual, expected) {
		t.Errorf("expected order %v, got %v", expected, actual)
	}

	// Test that original slice is not modified
	if testScratches[0].ID != "oldest" {
		t.Errorf("original slice was modified")
	}
}

func TestGetScratchByIndex(t *testing.T) {
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	now := time.Now()
	testScratches := []store.Scratch{
		{ID: "1", Project: "project1", Title: "Title 1", CreatedAt: now.Add(-3 * time.Hour)}, // oldest, will be index 3
		{ID: "2", Project: "project2", Title: "Title 2", CreatedAt: now.Add(-2 * time.Hour)}, // index 3 in all
		{ID: "3", Project: "global", Title: "Global Title", CreatedAt: now.Add(-1 * time.Hour)}, // index 2 in all
		{ID: "4", Project: "project1", Title: "Another P1", CreatedAt: now}, // newest, will be index 1
	}

	for _, scratch := range testScratches {
		s.AddScratch(scratch)
	}

	tests := []struct {
		name        string
		all         bool
		global      bool
		project     string
		indexStr    string
		expectedID  string
		expectError bool
	}{
		{
			name:        "get first scratch in project1 (newest)",
			all:         false,
			global:      false,
			project:     "project1",
			indexStr:    "1",
			expectedID:  "4", // newest project1 scratch
			expectError: false,
		},
		{
			name:        "get second scratch in project1 (oldest)",
			all:         false,
			global:      false,
			project:     "project1",
			indexStr:    "2",
			expectedID:  "1", // oldest project1 scratch
			expectError: false,
		},
		{
			name:        "get first scratch globally (newest overall)",
			all:         true,
			global:      false,
			project:     "",
			indexStr:    "1",
			expectedID:  "4", // newest overall
			expectError: false,
		},
		{
			name:        "get global scratch",
			all:         false,
			global:      true,
			project:     "",
			indexStr:    "1",
			expectedID:  "3", // only global scratch
			expectError: false,
		},
		{
			name:        "invalid index string",
			all:         false,
			global:      false,
			project:     "project1",
			indexStr:    "invalid",
			expectedID:  "",
			expectError: true,
		},
		{
			name:        "index out of range - too high",
			all:         false,
			global:      false,
			project:     "project1",
			indexStr:    "99",
			expectedID:  "",
			expectError: true,
		},
		{
			name:        "index out of range - zero",
			all:         false,
			global:      false,
			project:     "project1",
			indexStr:    "0",
			expectedID:  "",
			expectError: true,
		},
		{
			name:        "index out of range - negative",
			all:         false,
			global:      false,
			project:     "project1",
			indexStr:    "-1",
			expectedID:  "",
			expectError: true,
		},
		{
			name:        "no scratches in non-existent project",
			all:         false,
			global:      false,
			project:     "nonexistent",
			indexStr:    "1",
			expectedID:  "",
			expectError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result, err := GetScratchByIndex(s, tt.all, tt.global, tt.project, tt.indexStr)
			
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

func setupCommandsTestDir(t *testing.T) string {
	tmpDir := t.TempDir()
	oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
	os.Setenv("XDG_DATA_HOME", tmpDir)
	xdg.Reload()
	t.Cleanup(func() {
		if oldXDGDataHome == "" {
			os.Unsetenv("XDG_DATA_HOME")
		} else {
			os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
		}
		xdg.Reload()
	})

	scratchDir := filepath.Join(tmpDir, "scratch")
	if err := os.MkdirAll(scratchDir, 0755); err != nil {
		t.Fatalf("failed to create scratch directory: %v", err)
	}

	return tmpDir
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


func TestCreateWithEmptyContent(t *testing.T) {
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	// Set editor to a command that will write test content and exit
	oldEditor := os.Getenv("EDITOR")
	testScript := createMockEditorScript(t, "test content from editor")
	defer os.Remove(testScript)
	
	os.Setenv("EDITOR", testScript)
	defer func() {
		if oldEditor == "" {
			os.Unsetenv("EDITOR")
		} else {
			os.Setenv("EDITOR", oldEditor)
		}
	}()

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	initialCount := len(s.GetScratches())

	// Test with empty content - should launch editor
	err = Create(s, "testproject", []byte(""))
	if err != nil {
		t.Errorf("unexpected error: %v", err)
		return
	}

	scratches := s.GetScratches()
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