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
	singleCommands := []string{"create", "open", "peek", "view", "delete"}
	for _, cmd := range singleCommands {
		if !strings.Contains(output, cmd) {
			t.Errorf("Expected command '%s' in help output", cmd)
		}
	}

	// Check that multiple scratch commands are in the right group
	multipleCommands := []string{"ls", "cleanup", "nuke"}
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
	if !strings.Contains(output, "$ padz                  # Lists scratches with an index to be used in open, view, delete:") {
		t.Error("Expected usage example '$ padz' in help output")
	}
	if !strings.Contains(output, "$ padz create             # create a new scratch in $EDITOR") {
		t.Error("Expected usage example '$ padz create' in help output")
	}
	if !strings.Contains(output, "$ padz \"My scratch title. Can have content\"  # shortcut to create") {
		t.Error("Expected usage example '$ padz \"My scratch title...\"' in help output")
	}
	if !strings.Contains(output, "$ padz view <index>       # views in shell") {
		t.Error("Expected usage example '$ padz view <index>' in help output")
	}
	if !strings.Contains(output, "$ padz ls -s \"<term>\"     # search for scratches") {
		t.Error("Expected usage example '$ padz ls -s' in help output")
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

func TestShouldRunViewOrOpen(t *testing.T) {
	tests := []struct {
		name     string
		args     []string
		expected bool
	}{
		{
			name:     "single integer",
			args:     []string{"1"},
			expected: true,
		},
		{
			name:     "larger integer",
			args:     []string{"42"},
			expected: true,
		},
		{
			name:     "zero",
			args:     []string{"0"},
			expected: false, // Only positive integers
		},
		{
			name:     "negative integer",
			args:     []string{"-1"},
			expected: false, // Starts with - so will be treated as flag
		},
		{
			name:     "non-integer",
			args:     []string{"foo"},
			expected: false,
		},
		{
			name:     "multiple args with int",
			args:     []string{"1", "2"},
			expected: false,
		},
		{
			name:     "empty args",
			args:     []string{},
			expected: false,
		},
		{
			name:     "float",
			args:     []string{"1.5"},
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := shouldRunViewOrOpen(tt.args)
			if result != tt.expected {
				t.Errorf("shouldRunViewOrOpen(%v) = %v, want %v", tt.args, result, tt.expected)
			}
		})
	}
}

func TestShouldRunLs(t *testing.T) {
	tests := []struct {
		name     string
		args     []string
		expected bool
	}{
		{
			name:     "no args",
			args:     []string{},
			expected: true,
		},
		{
			name:     "search flag short",
			args:     []string{"-s", "term"},
			expected: true,
		},
		{
			name:     "search flag long",
			args:     []string{"--search=term"},
			expected: true,
		},
		{
			name:     "all flag",
			args:     []string{"-a"},
			expected: true,
		},
		{
			name:     "non-flag arg",
			args:     []string{"hello"},
			expected: false,
		},
		{
			name:     "integer arg",
			args:     []string{"123"},
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := shouldRunLs(tt.args)
			if result != tt.expected {
				t.Errorf("shouldRunLs(%v) = %v, want %v", tt.args, result, tt.expected)
			}
		})
	}
}

func TestShouldRunCreate(t *testing.T) {
	tests := []struct {
		name     string
		args     []string
		expected bool
	}{
		{
			name:     "multiple words",
			args:     []string{"hello", "world"},
			expected: true,
		},
		{
			name:     "quoted string",
			args:     []string{"hello world"},
			expected: true,
		},
		{
			name:     "known command",
			args:     []string{"ls"},
			expected: false,
		},
		{
			name:     "empty args",
			args:     []string{},
			expected: false,
		},
		{
			name:     "single integer",
			args:     []string{"42"},
			expected: false, // Will be caught by shouldRunViewOrOpen first
		},
		{
			name:     "create command",
			args:     []string{"create"},
			expected: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := shouldRunCreate(tt.args)
			if result != tt.expected {
				t.Errorf("shouldRunCreate(%v) = %v, want %v", tt.args, result, tt.expected)
			}
		})
	}
}
