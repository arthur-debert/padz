package commands

import (
	"bytes"
	"os/exec"
	"runtime"
	"testing"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

func TestCopy(t *testing.T) {
	// Skip on Windows as clipboard is not supported
	if runtime.GOOS == "windows" {
		t.Skip("Clipboard functionality not supported on Windows")
	}

	// Check if clipboard commands are available
	var clipboardCmd string
	switch runtime.GOOS {
	case "darwin":
		clipboardCmd = "pbpaste"
	case "linux":
		// Try xclip first, then xsel
		if _, err := exec.LookPath("xclip"); err == nil {
			clipboardCmd = "xclip"
		} else if _, err := exec.LookPath("xsel"); err == nil {
			clipboardCmd = "xsel"
		} else {
			t.Skip("Neither xclip nor xsel found in PATH")
		}
	default:
		t.Skip("Unsupported operating system for clipboard tests")
	}

	// Make sure the clipboard command actually works
	if _, err := exec.Command(clipboardCmd, "-help").CombinedOutput(); err != nil {
		t.Skipf("Clipboard command %s not working properly", clipboardCmd)
	}

	// Setup test environment
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	s := setup.Store

	// Create test scratches
	scratch1 := store.Scratch{
		ID:        "test1",
		Project:   "project1",
		Title:     "First scratch",
		CreatedAt: time.Now(),
	}
	scratch2 := store.Scratch{
		ID:        "test2",
		Project:   "project1",
		Title:     "Second scratch",
		CreatedAt: time.Now().Add(-1 * time.Hour),
	}
	scratch3 := store.Scratch{
		ID:        "test3",
		Project:   "project2",
		Title:     "Third scratch",
		CreatedAt: time.Now().Add(-2 * time.Hour),
	}

	// Add scratches to store
	if err := s.AddScratch(scratch1); err != nil {
		t.Fatalf("Failed to add scratch1: %v", err)
	}
	if err := s.AddScratch(scratch2); err != nil {
		t.Fatalf("Failed to add scratch2: %v", err)
	}
	if err := s.AddScratch(scratch3); err != nil {
		t.Fatalf("Failed to add scratch3: %v", err)
	}

	// Save content for each scratch
	content1 := []byte("Content of first scratch")
	content2 := []byte("Content of second scratch")
	content3 := []byte("Content of third scratch")

	if err := saveScratchFile(scratch1.ID, content1); err != nil {
		t.Fatalf("Failed to save content1: %v", err)
	}
	if err := saveScratchFile(scratch2.ID, content2); err != nil {
		t.Fatalf("Failed to save content2: %v", err)
	}
	if err := saveScratchFile(scratch3.ID, content3); err != nil {
		t.Fatalf("Failed to save content3: %v", err)
	}

	tests := []struct {
		name            string
		all             bool
		project         string
		indexStr        string
		expectedContent []byte
		expectError     bool
	}{
		{
			name:            "copy scratch by index in project",
			all:             false,
			project:         "project1",
			indexStr:        "1",
			expectedContent: content1,
			expectError:     false,
		},
		{
			name:            "copy scratch by index across all projects",
			all:             true,
			project:         "",
			indexStr:        "3",
			expectedContent: content3,
			expectError:     false,
		},
		{
			name:        "copy with invalid index",
			all:         false,
			project:     "project1",
			indexStr:    "99",
			expectError: true,
		},
		{
			name:        "copy with non-numeric index",
			all:         false,
			project:     "project1",
			indexStr:    "abc",
			expectError: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := Copy(s, tt.all, false, tt.project, tt.indexStr)
			if tt.expectError {
				if err == nil {
					t.Errorf("Expected error but got none")
				}
				return
			}

			if err != nil {
				t.Errorf("Unexpected error: %v", err)
				return
			}

			// Give clipboard operation time to complete
			time.Sleep(100 * time.Millisecond)

			// Read clipboard content
			var cmd *exec.Cmd
			switch runtime.GOOS {
			case "darwin":
				cmd = exec.Command("pbpaste")
			case "linux":
				if clipboardCmd == "xclip" {
					cmd = exec.Command("xclip", "-selection", "clipboard", "-o")
				} else {
					cmd = exec.Command("xsel", "--clipboard", "--output")
				}
			}

			output, err := cmd.Output()
			if err != nil {
				t.Fatalf("Failed to read clipboard: %v", err)
			}

			if !bytes.Equal(output, tt.expectedContent) {
				t.Errorf("Clipboard content mismatch\nExpected: %q\nGot: %q", tt.expectedContent, output)
			}
		})
	}
}

func TestCopyNonExistentScratch(t *testing.T) {
	// Setup test environment
	setup := SetupCommandTest(t)
	defer setup.Cleanup()

	s := setup.Store

	// Try to copy a scratch that doesn't exist
	err := Copy(s, false, false, "project1", "1")
	if err == nil {
		t.Errorf("Expected error when copying non-existent scratch, but got none")
	}
}
