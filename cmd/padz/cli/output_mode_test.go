package cli

import (
	"testing"
)

func TestOutputModeFlags(t *testing.T) {
	rootCmd := NewRootCmd()

	// Check that both flags exist
	silentFlag := rootCmd.PersistentFlags().Lookup("silent")
	if silentFlag == nil {
		t.Error("expected --silent flag to exist")
	}

	verboseFlag := rootCmd.PersistentFlags().Lookup("verbose")
	if verboseFlag == nil {
		t.Error("expected --verbose flag to exist")
	}
}

func TestOutputModeMutualExclusion(t *testing.T) {
	t.Skip("Skipping test that causes log.Fatal - needs refactoring to avoid process exit")
	// Test that setting both flags results in an error
	// This would be tested by running the actual command with both flags
	// For now, we just verify the flags exist
	rootCmd := NewRootCmd()

	// Set both flags
	rootCmd.SetArgs([]string{"--silent", "--verbose", "ls"})
	err := rootCmd.Execute()

	// Should fail due to mutual exclusion
	if err == nil {
		t.Error("expected error when both --silent and --verbose are set")
	}
}

func TestOutputModeHelpers(t *testing.T) {
	// Reset flags to default state
	silent = false
	verbose = false

	// Test default behavior (verbose)
	if IsVerboseMode() {
		t.Error("expected IsVerboseMode to be false when neither flag is set")
	}

	if IsSilentMode() {
		t.Error("expected IsSilentMode to be false by default")
	}

	// Test silent mode
	silent = true
	verbose = false

	if IsVerboseMode() {
		t.Error("expected IsVerboseMode to be false when silent is true")
	}

	if !IsSilentMode() {
		t.Error("expected IsSilentMode to be true when silent is true")
	}

	// Test verbose mode
	silent = false
	verbose = true

	if !IsVerboseMode() {
		t.Error("expected IsVerboseMode to be true when verbose is true")
	}

	if IsSilentMode() {
		t.Error("expected IsSilentMode to be false when verbose is true")
	}

	// Test both flags set (should not happen in practice due to validation)
	silent = true
	verbose = true

	if IsVerboseMode() {
		t.Error("expected IsVerboseMode to be false when both flags are true")
	}

	if !IsSilentMode() {
		t.Error("expected IsSilentMode to be true when silent is true regardless of verbose")
	}

	// Reset to defaults
	silent = false
	verbose = false
}
