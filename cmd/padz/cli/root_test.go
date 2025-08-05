package cli

import (
	"bytes"
	"strings"
	"testing"
)

func TestCommandGroups(t *testing.T) {
	cmd := NewRootCmd()
	buf := new(bytes.Buffer)
	cmd.SetOut(buf)
	cmd.SetArgs([]string{"--help"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	output := buf.String()

	// Check for command groups
	if !strings.Contains(output, "SINGLE SCRATCH:") {
		t.Error("Expected 'SINGLE SCRATCH:' group in help output")
	}

	if !strings.Contains(output, "SCRATCHES:") {
		t.Error("Expected 'SCRATCHES:' group in help output")
	}

	// Check that single scratch commands are in the right group
	singleCommands := []string{"open", "peek", "view", "delete"}
	for _, cmd := range singleCommands {
		if !strings.Contains(output, cmd) {
			t.Errorf("Expected command '%s' in help output", cmd)
		}
	}

	// Check that multiple scratch commands are in the right group
	multipleCommands := []string{"ls", "cleanup", "search"}
	for _, cmd := range multipleCommands {
		if !strings.Contains(output, cmd) {
			t.Errorf("Expected command '%s' in help output", cmd)
		}
	}

	// Check for $EDITOR replacement
	if !strings.Contains(output, "$EDITOR") {
		t.Error("Expected '$EDITOR' in help output instead of 'default editor'")
	}

	// Commands in main help don't show parameters, only in individual help

	// Check for usage examples
	if !strings.Contains(output, "$ padz                    # edit a new scratch in $EDITOR") {
		t.Error("Expected usage example '$ padz' in help output")
	}
	if !strings.Contains(output, "$ padz ls                 # Lists scratches") {
		t.Error("Expected usage example '$ padz ls' in help output")
	}
	if !strings.Contains(output, "$ padz view <index>       # views in shell") {
		t.Error("Expected usage example '$ padz view <index>' in help output")
	}
	if !strings.Contains(output, "$ padz search \"<term>\"    # search for scratches") {
		t.Error("Expected usage example '$ padz search' in help output")
	}
}

func TestVersionFlag(t *testing.T) {
	cmd := NewRootCmd()
	buf := new(bytes.Buffer)
	// Capture both stdout and stderr
	cmd.SetOut(buf)
	cmd.SetErr(buf)
	cmd.SetArgs([]string{"--version"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	output := buf.String()

	// Check version output format
	if !strings.Contains(output, "padz version") {
		t.Errorf("Expected 'padz version' in version output, got: %s", output)
	}
	if !strings.Contains(output, "(commit:") {
		t.Errorf("Expected '(commit:' in version output, got: %s", output)
	}
	if !strings.Contains(output, "built:") {
		t.Errorf("Expected 'built:' in version output, got: %s", output)
	}
}

func TestVersionSubcommandRemoved(t *testing.T) {
	cmd := NewRootCmd()
	buf := new(bytes.Buffer)
	cmd.SetErr(buf)
	cmd.SetArgs([]string{"version"})

	// Should fail with unknown command
	err := cmd.Execute()
	if err == nil {
		t.Error("Expected error when using 'version' as subcommand")
	}

	errOutput := buf.String()
	if !strings.Contains(errOutput, "unknown command") {
		t.Error("Expected 'unknown command' error message")
	}
}

func TestVerboseFlagHidden(t *testing.T) {
	cmd := NewRootCmd()
	buf := new(bytes.Buffer)
	cmd.SetOut(buf)
	cmd.SetArgs([]string{"--help"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	output := buf.String()

	// Check that verbose flag is not in help
	if strings.Contains(output, "-v, --verbose") {
		t.Error("Verbose flag should be hidden from help output")
	}
	if strings.Contains(output, "Increase verbosity") {
		t.Error("Verbose flag description should be hidden from help output")
	}
}

func TestVerboseFlagStillWorks(t *testing.T) {
	// Test that hidden flag still functions
	cmd := NewRootCmd()
	cmd.SetArgs([]string{"-v", "--help"})

	// Should not error
	err := cmd.Execute()
	if err != nil {
		t.Fatalf("unexpected error when using hidden verbose flag: %v", err)
	}
}

func TestFormatFlag(t *testing.T) {
	cmd := NewRootCmd()
	buf := new(bytes.Buffer)
	cmd.SetOut(buf)
	cmd.SetArgs([]string{"--help"})

	err := cmd.Execute()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}

	output := buf.String()

	// Check that format flag is shown
	if !strings.Contains(output, "--format") {
		t.Error("Expected --format flag in help output")
	}
	if !strings.Contains(output, "Output format") {
		t.Error("Expected format flag description in help output")
	}
	if !strings.Contains(output, "plain, json, term") {
		t.Error("Expected format options in help output")
	}
}
