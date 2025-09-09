package cli

import (
	"testing"

	"github.com/spf13/cobra"
)

func TestShowDataFileCommand(t *testing.T) {
	// Test that the command is properly configured
	cmd := newShowDataFileCmd()

	if cmd.Use != ShowDataFileUse {
		t.Errorf("expected Use to be %q, got %q", ShowDataFileUse, cmd.Use)
	}

	if cmd.Short != ShowDataFileShort {
		t.Errorf("expected Short to be %q, got %q", ShowDataFileShort, cmd.Short)
	}

	if cmd.Long != ShowDataFileLong {
		t.Errorf("expected Long to be %q, got %q", ShowDataFileLong, cmd.Long)
	}

	// Test that it expects no arguments
	if err := cmd.Args(cmd, []string{}); err != nil {
		t.Errorf("unexpected error with no arguments: %v", err)
	}

	if err := cmd.Args(cmd, []string{"extra"}); err == nil {
		t.Error("expected error with extra argument")
	}
}

func TestShowDataFileCommandFlags(t *testing.T) {
	// The flag is added by root.go when integrating the command
	rootCmd := NewRootCmd()

	// Find the show-data-file command
	var showDataFileCmd *cobra.Command
	for _, cmd := range rootCmd.Commands() {
		if cmd.Name() == "show-data-file" {
			showDataFileCmd = cmd
			break
		}
	}

	if showDataFileCmd == nil {
		t.Fatal("show-data-file command not found")
	}

	// Check that it has the global flag
	globalFlag := showDataFileCmd.Flag("global")
	if globalFlag == nil {
		t.Error("expected show-data-file command to have --global flag")
	}
}

func TestShowDataFileCommandRun(t *testing.T) {
	// Test that the Run function is defined
	cmd := newShowDataFileCmd()
	if cmd.Run == nil {
		t.Error("expected Run function to be defined")
	}
}
