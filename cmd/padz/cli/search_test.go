package cli

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestSearchCommand(t *testing.T) {
	// Test that the search command is properly configured
	cmd := newSearchCmd()

	assert.Equal(t, "search [term]", cmd.Use)
	assert.Equal(t, "Search for scratches containing the given term", cmd.Short)
	assert.NotEmpty(t, cmd.Long)

	// Test that it expects at least one argument
	assert.NotNil(t, cmd.Args)
}

func TestSearchCommandInRoot(t *testing.T) {
	// Test that search command is properly registered in root
	rootCmd := NewRootCmd()

	// Find the search command
	searchCmd, _, err := rootCmd.Find([]string{"search"})
	assert.NoError(t, err)
	assert.NotNil(t, searchCmd)
	assert.Equal(t, "search", searchCmd.Name())

	// Verify it's in the multiple scratches group
	assert.Equal(t, "multiple", searchCmd.GroupID)
}

func TestSearchCommandArgumentHandling(t *testing.T) {
	tests := []struct {
		name        string
		args        []string
		shouldError bool
	}{
		{
			name:        "no arguments",
			args:        []string{"search"},
			shouldError: true,
		},
		{
			name:        "single argument",
			args:        []string{"search", "test"},
			shouldError: false,
		},
		{
			name:        "multiple arguments",
			args:        []string{"search", "test", "note"},
			shouldError: false,
		},
		{
			name:        "with flags",
			args:        []string{"search", "--all", "test"},
			shouldError: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			rootCmd := NewRootCmd()
			rootCmd.SetArgs(tt.args)

			// We can't easily test the full execution without a proper store setup,
			// but we can validate that the command parsing works
			cmd, _, err := rootCmd.Find(tt.args)

			if tt.shouldError {
				// For "no arguments", the command will be found but args validation should fail
				if cmd != nil && cmd.Name() == "search" {
					// Manually validate args since we can't run the full command
					err = cmd.Args(cmd, tt.args[1:])
					assert.Error(t, err, "Expected args validation to fail")
				}
			} else {
				assert.NoError(t, err)
				assert.NotNil(t, cmd)
			}
		})
	}
}
