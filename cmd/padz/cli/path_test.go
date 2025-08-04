package cli

import (
	"bytes"
	"strings"
	"testing"
)

func TestPathCommand(t *testing.T) {
	// Test that the path command is available
	cmd := NewRootCmd()
	
	// Look for path command in the command list
	pathCmd, _, err := cmd.Find([]string{"path"})
	if err != nil {
		t.Fatalf("path command not found: %v", err)
	}
	
	if pathCmd.Use != "path <index>" {
		t.Errorf("expected 'path <index>', got '%s'", pathCmd.Use)
	}
	
	if pathCmd.Short != "Get the full path to a scratch" {
		t.Errorf("unexpected short description: %s", pathCmd.Short)
	}
	
	// Test that it's in the single scratch group
	if pathCmd.GroupID != "single" {
		t.Errorf("expected path command to be in 'single' group, got '%s'", pathCmd.GroupID)
	}
}

func TestPathCommandInHelp(t *testing.T) {
	cmd := NewRootCmd()
	buf := new(bytes.Buffer)
	cmd.SetOut(buf)
	cmd.SetArgs([]string{"--help"})
	
	err := cmd.Execute()
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	
	output := buf.String()
	
	// Check that path command appears in help under SINGLE SCRATCH
	if !strings.Contains(output, "path") {
		t.Error("Expected 'path' command in help output")
	}
	
	// Check it's in the right section
	lines := strings.Split(output, "\n")
	inSingleSection := false
	foundPath := false
	
	for _, line := range lines {
		if strings.Contains(line, "SINGLE SCRATCH:") {
			inSingleSection = true
		} else if strings.Contains(line, "SCRATCHES:") {
			inSingleSection = false
		}
		
		if inSingleSection && strings.Contains(line, "path") {
			foundPath = true
			break
		}
	}
	
	if !foundPath {
		t.Error("Expected 'path' command to be in SINGLE SCRATCH section")
	}
}