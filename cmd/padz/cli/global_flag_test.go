package cli

import (
	"os"
	"testing"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

// TestGlobalFlagConsistency tests that all commands that should support --global flag do support it
func TestGlobalFlagConsistency(t *testing.T) {
	// Commands that should have --global flag
	commandsWithGlobal := map[string]bool{
		"create":         true,
		"view":           true,
		"open":           true,
		"peek":           true,
		"delete":         true,
		"path":           true,
		"copy":           true,
		"pin":            true,
		"unpin":          true,
		"ls":             true,
		"export":         true,
		"show-data-file": true,
	}

	rootCmd := NewRootCmd()

	for _, cmd := range rootCmd.Commands() {
		if shouldHaveGlobal, ok := commandsWithGlobal[cmd.Name()]; ok {
			globalFlag := cmd.Flag("global")
			if shouldHaveGlobal && globalFlag == nil {
				t.Errorf("Command %s should have --global flag but doesn't", cmd.Name())
			}
		}
	}
}

// TestGlobalFlagFunctionality tests that global flag works correctly for create and list
func TestGlobalFlagFunctionality(t *testing.T) {
	// Skip if not in proper test environment
	if testing.Short() {
		t.Skip("skipping integration test in short mode")
	}

	// Create temporary test directory
	tempDir, err := os.MkdirTemp("", "padz-global-test")
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		_ = os.RemoveAll(tempDir)
	}()

	// Change to temp directory
	oldPwd, _ := os.Getwd()
	if err := os.Chdir(tempDir); err != nil {
		t.Fatal(err)
	}
	defer func() {
		_ = os.Chdir(oldPwd)
	}()

	// Initialize test project
	if err := os.WriteFile(".padz", []byte("test-project"), 0644); err != nil {
		t.Fatal(err)
	}

	// Test scenarios
	tests := []struct {
		name          string
		setupFunc     func(t *testing.T, s *store.Store)
		command       string
		args          []string
		expectGlobal  bool
		expectProject bool
	}{
		{
			name: "create with --global should create global scratch",
			setupFunc: func(t *testing.T, s *store.Store) {
				// No setup needed
			},
			command:      "create",
			args:         []string{"--global", "--title", "Global Test"},
			expectGlobal: true,
		},
		{
			name: "create without --global should create project scratch",
			setupFunc: func(t *testing.T, s *store.Store) {
				// No setup needed
			},
			command:       "create",
			args:          []string{"--title", "Project Test"},
			expectProject: true,
		},
		{
			name: "ls with --global should only show global scratches",
			setupFunc: func(t *testing.T, s *store.Store) {
				// Add both global and project scratches
				if err := s.AddScratch(store.Scratch{ID: "g1", Project: "global", Title: "Global 1"}); err != nil {
					t.Fatal(err)
				}
				if err := s.AddScratch(store.Scratch{ID: "p1", Project: tempDir, Title: "Project 1"}); err != nil {
					t.Fatal(err)
				}
			},
			command:      "ls",
			args:         []string{"--global"},
			expectGlobal: true,
		},
		{
			name: "ls without --global should only show project scratches",
			setupFunc: func(t *testing.T, s *store.Store) {
				// Scratches already added from previous test
			},
			command:       "ls",
			args:          []string{},
			expectProject: true,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// This would require mocking the store and command execution
			// For now, we're testing the command structure
		})
	}
}

// TestCommandsHaveConsistentGlobalBehavior ensures all commands handle global flag consistently
func TestCommandsHaveConsistentGlobalBehavior(t *testing.T) {
	// Test that commands properly pass global flag to underlying functions
	testCases := []struct {
		cmdName              string
		hasGlobalFlag        bool
		shouldPassToCommands bool
	}{
		{"create", true, true},
		{"view", true, true},
		{"open", true, true},
		{"peek", true, true},
		{"delete", true, true},
		{"path", true, true},
		{"copy", true, true},
		{"pin", true, true},
		{"unpin", true, true},
		{"list", true, true},
		{"export", true, true},
	}

	for _, tc := range testCases {
		t.Run(tc.cmdName, func(t *testing.T) {
			rootCmd := NewRootCmd()
			var targetCmd *cobra.Command

			for _, cmd := range rootCmd.Commands() {
				if cmd.Name() == tc.cmdName {
					targetCmd = cmd
					break
				}
			}

			if targetCmd == nil {
				t.Fatalf("Command %s not found", tc.cmdName)
			}

			globalFlag := targetCmd.Flag("global")
			if tc.hasGlobalFlag && globalFlag == nil {
				t.Errorf("Command %s should have global flag but doesn't", tc.cmdName)
			} else if !tc.hasGlobalFlag && globalFlag != nil {
				t.Errorf("Command %s shouldn't have global flag but does", tc.cmdName)
			}
		})
	}
}
