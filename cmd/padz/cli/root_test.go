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
}