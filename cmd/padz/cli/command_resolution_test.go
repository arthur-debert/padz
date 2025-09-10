package cli

import (
	"testing"

	"github.com/stretchr/testify/assert"
)

func TestResolveCommand(t *testing.T) {
	// Note: config.NakedIntCommand is a const set to "view"

	tests := []struct {
		name     string
		args     []string
		expected []string
	}{
		// Case 1: No arguments -> list command
		{
			name:     "no arguments",
			args:     []string{},
			expected: []string{"list"},
		},

		// Case 2: Single integer -> view command
		{
			name:     "single integer",
			args:     []string{"5"},
			expected: []string{"view", "5"},
		},
		{
			name:     "single large integer",
			args:     []string{"123"},
			expected: []string{"view", "123"},
		},
		{
			name:     "zero is not valid",
			args:     []string{"0"},
			expected: []string{"create", "0"},
		},
		{
			name:     "negative integer is not valid",
			args:     []string{"-5"},
			expected: []string{"list", "-5"}, // Treated as a flag, goes to list
		},

		// Case 3: Known commands pass through
		{
			name:     "list command",
			args:     []string{"list"},
			expected: []string{"list"},
		},
		{
			name:     "create command",
			args:     []string{"create", "hello"},
			expected: []string{"create", "hello"},
		},
		{
			name:     "help command",
			args:     []string{"help"},
			expected: []string{"help"},
		},
		{
			name:     "command with flags",
			args:     []string{"list", "-a"},
			expected: []string{"list", "-a"},
		},
		{
			name:     "command alias",
			args:     []string{"ls"},
			expected: []string{"ls"},
		},
		{
			name:     "delete alias",
			args:     []string{"rm", "5"},
			expected: []string{"rm", "5"},
		},

		// Case 4: Unknown args -> create command
		{
			name:     "single word",
			args:     []string{"hello"},
			expected: []string{"create", "hello"},
		},
		{
			name:     "multiple words",
			args:     []string{"Once", "upon", "a", "time"},
			expected: []string{"create", "Once", "upon", "a", "time"},
		},
		{
			name:     "with title flag",
			args:     []string{"-t", "My Title"},
			expected: []string{"create", "-t", "My Title"},
		},
		{
			name:     "quoted string",
			args:     []string{"Hello world"},
			expected: []string{"create", "Hello world"},
		},

		// Edge cases
		{
			name:     "help flag alone",
			args:     []string{"--help"},
			expected: []string{"--help"},
		},
		{
			name:     "version flag alone",
			args:     []string{"--version"},
			expected: []string{"--version"},
		},
		{
			name:     "float is not integer",
			args:     []string{"5.5"},
			expected: []string{"create", "5.5"},
		},
		{
			name:     "list with search flag",
			args:     []string{"-s", "term"},
			expected: []string{"list", "-s", "term"},
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			result := resolveCommand(tt.args)
			assert.Equal(t, tt.expected, result,
				"resolveCommand(%v) = %v, want %v", tt.args, result, tt.expected)
		})
	}
}
