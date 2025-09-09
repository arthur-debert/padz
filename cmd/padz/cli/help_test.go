package cli

import (
	"bytes"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func TestHelpCommand(t *testing.T) {
	tests := []struct {
		name             string
		args             []string
		shouldContain    []string
		shouldNotContain []string
	}{
		{
			name: "padz help shows root help",
			args: []string{"help"},
			shouldContain: []string{
				"padz create scratch pads",
				"SINGLE SCRATCH:",
				"SCRATCHES:",
				"create",
				"ls",
			},
			shouldNotContain: []string{
				"Search results are ranked by:", // This is from ls detailed help
			},
		},
		{
			name: "padz --help shows root help",
			args: []string{"--help"},
			shouldContain: []string{
				"padz create scratch pads",
				"SINGLE SCRATCH:",
				"SCRATCHES:",
				"create",
				"ls",
			},
			shouldNotContain: []string{
				"Search results are ranked by:", // This is from ls detailed help
			},
		},
		{
			name: "padz -h shows root help",
			args: []string{"-h"},
			shouldContain: []string{
				"padz create scratch pads",
				"SINGLE SCRATCH:",
				"SCRATCHES:",
			},
		},
		{
			name: "command help shows aliases",
			args: []string{"help", "create"},
			shouldContain: []string{
				"Aliases:",
				"create, new, n",
			},
		},
		{
			name: "command help shows flags",
			args: []string{"help", "ls"},
			shouldContain: []string{
				"Flags:",
				"--all",
				"--global",
				"--search",
			},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Create root command
			rootCmd := NewRootCmd()

			// Capture output
			buf := new(bytes.Buffer)
			rootCmd.SetOut(buf)
			rootCmd.SetErr(buf)

			// Simulate Execute() logic for help flags
			args := tt.args
			if len(args) > 0 && (args[0] == "--help" || args[0] == "-h") {
				// Don't modify args for help flags
			} else if shouldRunLs(args) {
				args = append([]string{"ls"}, args...)
			}

			rootCmd.SetArgs(args)
			err := rootCmd.Execute()

			// Help command returns no error
			require.NoError(t, err)

			output := buf.String()

			// Check expected content
			for _, expected := range tt.shouldContain {
				assert.Contains(t, output, expected,
					"Expected help output to contain %q", expected)
			}

			// Check unexpected content
			for _, unexpected := range tt.shouldNotContain {
				assert.NotContains(t, output, unexpected,
					"Expected help output NOT to contain %q", unexpected)
			}
		})
	}
}

func TestHelpCommandEquivalence(t *testing.T) {
	// Test that both help forms produce identical output
	rootCmd1 := NewRootCmd()
	buf1 := new(bytes.Buffer)
	rootCmd1.SetOut(buf1)
	rootCmd1.SetErr(buf1)
	rootCmd1.SetArgs([]string{"help"})

	err := rootCmd1.Execute()
	require.NoError(t, err)
	output1 := buf1.String()

	// Test padz --help
	rootCmd2 := NewRootCmd()
	buf2 := new(bytes.Buffer)
	rootCmd2.SetOut(buf2)
	rootCmd2.SetErr(buf2)
	rootCmd2.SetArgs([]string{"--help"})

	err = rootCmd2.Execute()
	require.NoError(t, err)
	output2 := buf2.String()

	// Remove any trailing whitespace differences
	output1 = strings.TrimSpace(output1)
	output2 = strings.TrimSpace(output2)

	assert.Equal(t, output1, output2,
		"'padz help' and 'padz --help' should produce identical output")
}
