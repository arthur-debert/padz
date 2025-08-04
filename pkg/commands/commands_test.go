package commands

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
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
			name:        "create with empty content",
			project:     "testproject",
			content:     []byte(""),
			expectError: false,
			expectSave:  false,
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
	t.Skip("ReadContentFromPipe requires stdin manipulation which is complex to test")
}

func TestLs(t *testing.T) {
	tmpDir := setupCommandsTestDir(t)
	defer os.RemoveAll(tmpDir)

	s, err := store.NewStore()
	if err != nil {
		t.Fatalf("failed to create store: %v", err)
	}

	testScratches := []store.Scratch{
		{ID: "1", Project: "project1", Title: "Title 1", CreatedAt: time.Now()},
		{ID: "2", Project: "project2", Title: "Title 2", CreatedAt: time.Now()},
		{ID: "3", Project: "global", Title: "Global Title", CreatedAt: time.Now()},
		{ID: "4", Project: "project1", Title: "Another P1", CreatedAt: time.Now()},
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
			name:          "list all scratches",
			all:           true,
			global:        false,
			project:       "",
			expectedCount: 4,
			expectedIDs:   []string{"1", "2", "3", "4"},
		},
		{
			name:          "list project1 scratches",
			all:           false,
			global:        false,
			project:       "project1",
			expectedCount: 2,
			expectedIDs:   []string{"1", "4"},
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

	testScratches := []store.Scratch{
		{ID: "1", Project: "project1", Title: "Title 1", CreatedAt: time.Now()},
		{ID: "2", Project: "project2", Title: "Title 2", CreatedAt: time.Now()},
		{ID: "3", Project: "global", Title: "Global Title", CreatedAt: time.Now()},
	}

	testContents := map[string]string{
		"1": "This is content for scratch 1\nWith some test data",
		"2": "Different content for scratch 2\nNo test here",
		"3": "Global scratch content\nTesting global search",
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
			expectedCount: 2,
			expectedIDs:   []string{"1", "3"},
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
			name:          "regex pattern search",
			all:           true,
			global:        false,
			project:       "",
			term:          "scratch [0-9]",
			expectedCount: 2,
			expectedIDs:   []string{"1", "2"},
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

	testScratch := store.Scratch{
		ID: "test1", Project: "testproject", Title: "Test Title", CreatedAt: time.Now(),
	}
	s.AddScratch(testScratch)

	tests := []struct {
		name            string
		content         string
		lines           int
		expectedContent string
	}{
		{
			name:            "short content - return all",
			content:         "Line 1\nLine 2\nLine 3",
			lines:           3,
			expectedContent: "Line 1\nLine 2\nLine 3",
		},
		{
			name:    "long content - peek with ellipsis",
			content: "Line 1\nLine 2\nLine 3\nLine 4\nLine 5\nLine 6\nLine 7\nLine 8",
			lines:   2,
			expectedContent: "Line 1\nLine 2\n...\nLine 7\nLine 8\n",
		},
		{
			name:            "single line content",
			content:         "Single line",
			lines:           3,
			expectedContent: "Single line",
		},
		{
			name:            "exactly 2*lines content",
			content:         "Line 1\nLine 2\nLine 3\nLine 4",
			lines:           2,
			expectedContent: "Line 1\nLine 2\nLine 3\nLine 4",
		},
		{
			name:            "empty content",
			content:         "",
			lines:           3,
			expectedContent: "",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			saveScratchFile(testScratch.ID, []byte(tt.content))

			result, err := Peek(s, false, false, "testproject", "1", tt.lines)
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

			err := Delete(s, tt.project, tt.indexStr)
			
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
		{ID: "new2", Project: "test", Title: "New 2", CreatedAt: now.AddDate(0, 0, -1)},
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
	t.Skip("Open function tests require editor interaction - complex to test without mocking")
}

func setupCommandsTestDir(t *testing.T) string {
	tmpDir := t.TempDir()
	oldXDGDataHome := os.Getenv("XDG_DATA_HOME")
	os.Setenv("XDG_DATA_HOME", tmpDir)
	t.Cleanup(func() {
		os.Setenv("XDG_DATA_HOME", oldXDGDataHome)
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