package project

import (
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func TestGetCurrentProject(t *testing.T) {
	tests := []struct {
		name            string
		setupDirs       []string
		startDir        string
		expectedProject string
		expectError     bool
	}{
		{
			name:            "find git repo in current directory",
			setupDirs:       []string{"myproject/.git"},
			startDir:        "myproject",
			expectedProject: "myproject",
			expectError:     false,
		},
		{
			name:            "find git repo in parent directory",
			setupDirs:       []string{"myproject/.git", "myproject/subdir"},
			startDir:        "myproject/subdir",
			expectedProject: "myproject",
			expectError:     false,
		},
		{
			name:            "find git repo multiple levels up",
			setupDirs:       []string{"myproject/.git", "myproject/src", "myproject/src/main"},
			startDir:        "myproject/src/main",
			expectedProject: "myproject",
			expectError:     false,
		},
		{
			name:            "no git repo found - return global",
			setupDirs:       []string{"nogitstuff"},
			startDir:        "nogitstuff",
			expectedProject: "global",
			expectError:     false,
		},
		{
			name:            "nested git repos - find closest",
			setupDirs:       []string{"outer/.git", "outer/inner/.git", "outer/inner/deep"},
			startDir:        "outer/inner/deep",
			expectedProject: "inner",
			expectError:     false,
		},
		{
			name:            "complex nested structure",
			setupDirs:       []string{"workspace/.git", "workspace/projects", "workspace/projects/app1", "workspace/projects/app1/src"},
			startDir:        "workspace/projects/app1/src",
			expectedProject: "workspace",
			expectError:     false,
		},
		{
			name:            "git repo at filesystem root level simulation",
			setupDirs:       []string{"rootlevel/.git"},
			startDir:        "rootlevel",
			expectedProject: "rootlevel",
			expectError:     false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tmpDir := setupProjectTestDir(t, tt.setupDirs)
			defer os.RemoveAll(tmpDir)

			startPath := filepath.Join(tmpDir, tt.startDir)
			project, err := GetCurrentProject(startPath)

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

			if project != tt.expectedProject {
				t.Errorf("expected project '%s', got '%s'", tt.expectedProject, project)
			}
		})
	}
}

func TestGetCurrentProject_EdgeCases(t *testing.T) {
	tests := []struct {
		name        string
		setupFunc   func(string) string
		expectedResult string
	}{
		{
			name: "directory with special characters",
			setupFunc: func(tmpDir string) string {
				specialDir := filepath.Join(tmpDir, "my-project_v2.0")
				gitDir := filepath.Join(specialDir, ".git")
				os.MkdirAll(gitDir, 0755)
				return specialDir
			},
			expectedResult: "my-project_v2.0",
		},
		{
			name: "directory with spaces",
			setupFunc: func(tmpDir string) string {
				spaceDir := filepath.Join(tmpDir, "my project")
				gitDir := filepath.Join(spaceDir, ".git")
				os.MkdirAll(gitDir, 0755)
				return spaceDir
			},
			expectedResult: "my project",
		},
		{
			name: "very deep nested structure",
			setupFunc: func(tmpDir string) string {
				deepPath := tmpDir
				for i := 0; i < 10; i++ {
					deepPath = filepath.Join(deepPath, fmt.Sprintf("level%d", i))
				}
				os.MkdirAll(deepPath, 0755)
				
				gitDir := filepath.Join(tmpDir, "level0", ".git")
				os.MkdirAll(gitDir, 0755)
				return deepPath
			},
			expectedResult: "level0",
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			tmpDir := t.TempDir()
			startPath := tt.setupFunc(tmpDir)

			project, err := GetCurrentProject(startPath)
			if err != nil {
				t.Errorf("unexpected error: %v", err)
				return
			}

			if project != tt.expectedResult {
				t.Errorf("expected project '%s', got '%s'", tt.expectedResult, project)
			}
		})
	}
}

func TestGetCurrentProject_NonExistentDirectory(t *testing.T) {
	nonExistentPath := "/this/path/does/not/exist"
	
	project, err := GetCurrentProject(nonExistentPath)
	if err != nil {
		t.Errorf("unexpected error for non-existent directory: %v", err)
	}
	
	if project != "global" {
		t.Errorf("expected 'global' for non-existent directory, got '%s'", project)
	}
}

func TestGetCurrentProject_GitFileNotDirectory(t *testing.T) {
	tmpDir := t.TempDir()
	projectDir := filepath.Join(tmpDir, "project")
	os.MkdirAll(projectDir, 0755)
	
	gitFile := filepath.Join(projectDir, ".git")
	if err := os.WriteFile(gitFile, []byte("gitdir: ../other.git"), 0644); err != nil {
		t.Fatalf("failed to create .git file: %v", err)
	}

	project, err := GetCurrentProject(projectDir)
	if err != nil {
		t.Errorf("unexpected error: %v", err)
	}

	if project != "project" {
		t.Errorf("expected 'project' when .git is a file (valid git worktree), got '%s'", project)
	}
}

func TestGetCurrentProject_SymlinkHandling(t *testing.T) {
	tmpDir := t.TempDir()
	realProject := filepath.Join(tmpDir, "realproject")
	gitDir := filepath.Join(realProject, ".git")
	os.MkdirAll(gitDir, 0755)

	linkProject := filepath.Join(tmpDir, "linkproject")
	if err := os.Symlink(realProject, linkProject); err != nil {
		t.Skip("symlinks not supported on this system")
	}

	project, err := GetCurrentProject(linkProject)
	if err != nil {
		t.Errorf("unexpected error with symlink: %v", err)
	}

	if project != "linkproject" {
		t.Errorf("expected 'linkproject' (symlink name), got '%s'", project)
	}
}

func BenchmarkGetCurrentProject(b *testing.B) {
	tmpDir := b.TempDir()
	
	deepPath := tmpDir
	for i := 0; i < 20; i++ {
		deepPath = filepath.Join(deepPath, fmt.Sprintf("level%d", i))
	}
	os.MkdirAll(deepPath, 0755)
	
	gitDir := filepath.Join(tmpDir, ".git")
	os.MkdirAll(gitDir, 0755)

	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		GetCurrentProject(deepPath)
	}
}

func setupProjectTestDir(t *testing.T, dirs []string) string {
	tmpDir := t.TempDir()

	for _, dir := range dirs {
		fullPath := filepath.Join(tmpDir, dir)
		if err := os.MkdirAll(fullPath, 0755); err != nil {
			t.Fatalf("failed to create directory %s: %v", fullPath, err)
		}
	}

	return tmpDir
}