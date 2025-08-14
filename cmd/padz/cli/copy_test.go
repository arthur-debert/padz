package cli

import (
	"bytes"
	"runtime"
	"strings"
	"testing"
)

func TestCopyCommand(t *testing.T) {
	// Skip on Windows as clipboard is not supported
	if runtime.GOOS == "windows" {
		t.Skip("Clipboard functionality not supported on Windows")
	}

	// Test that the copy command is available
	cmd := NewRootCmd()

	// Look for copy command in the command list
	copyCmd, _, err := cmd.Find([]string{"copy"})
	if err != nil {
		t.Fatalf("copy command not found: %v", err)
	}

	// Test command properties
	if copyCmd.Use != CopyUse {
		t.Errorf("Expected Use to be %q, got %q", CopyUse, copyCmd.Use)
	}

	if copyCmd.Short != CopyShort {
		t.Errorf("Expected Short to be %q, got %q", CopyShort, copyCmd.Short)
	}

	if copyCmd.Long != CopyLong {
		t.Errorf("Expected Long to be %q, got %q", CopyLong, copyCmd.Long)
	}

	// Test aliases
	expectedAliases := []string{"cp"}
	if len(copyCmd.Aliases) != len(expectedAliases) {
		t.Errorf("Expected %d aliases, got %d", len(expectedAliases), len(copyCmd.Aliases))
	}
	for i, alias := range expectedAliases {
		if i >= len(copyCmd.Aliases) || copyCmd.Aliases[i] != alias {
			t.Errorf("Expected alias[%d] to be %q, got %q", i, alias, copyCmd.Aliases[i])
		}
	}

	// Test that it's in the single scratch group
	if copyCmd.GroupID != "single" {
		t.Errorf("expected copy command to be in 'single' group, got '%s'", copyCmd.GroupID)
	}

	// Test args validation
	if copyCmd.Args == nil {
		t.Error("Expected Args to be set")
	}

	// Test that it requires exactly one argument
	if err := copyCmd.Args(copyCmd, []string{}); err == nil {
		t.Error("Expected error with no arguments")
	}
	if err := copyCmd.Args(copyCmd, []string{"1", "2"}); err == nil {
		t.Error("Expected error with multiple arguments")
	}
	if err := copyCmd.Args(copyCmd, []string{"1"}); err != nil {
		t.Errorf("Expected no error with one argument, got: %v", err)
	}

	// Test flags
	allFlag := copyCmd.Flag("all")
	if allFlag == nil {
		t.Error("Expected 'all' flag to be defined")
	} else if allFlag.Shorthand != "a" {
		t.Errorf("Expected 'all' flag shorthand to be 'a', got %q", allFlag.Shorthand)
	}
}

func TestCopyCommandInHelp(t *testing.T) {
	cmd := NewRootCmd()
	buf := new(bytes.Buffer)
	cmd.SetOut(buf)
	cmd.SetArgs([]string{"--help"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	output := buf.String()

	// Check that copy command appears in help
	if !strings.Contains(output, "copy") {
		t.Error("Expected 'copy' command in help output")
	}

	// Check it's in the right section
	lines := strings.Split(output, "\n")
	inSingleSection := false
	foundCopy := false

	for _, line := range lines {
		if strings.Contains(line, "SINGLE SCRATCH:") {
			inSingleSection = true
		} else if strings.Contains(line, "SCRATCHES:") {
			inSingleSection = false
		}

		if inSingleSection && strings.Contains(line, "copy") {
			foundCopy = true
			break
		}
	}

	if !foundCopy {
		t.Error("Expected 'copy' command to be in SINGLE SCRATCH section")
	}
}
