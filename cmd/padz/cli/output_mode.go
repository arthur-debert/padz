package cli

import (
	"fmt"

	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
)

// IsVerboseMode returns true if verbose output is enabled
func IsVerboseMode() bool {
	return verbose && !silent
}

// IsSilentMode returns true if silent output is enabled
func IsSilentMode() bool {
	return silent
}

// ShowListAfterCommand displays the list of scratches after a command if verbose mode is enabled
func ShowListAfterCommand(s *store.Store, global bool, project string) {
	if !IsVerboseMode() {
		return
	}

	// Get the list of scratches
	scratches := commands.Ls(s, global, project)

	// Format and display based on output format
	format, err := output.GetFormat(outputFormat)
	if err != nil {
		log.Debug().Err(err).Msg("Failed to get output format")
		return
	}

	switch format {
	case output.JSONFormat:
		// Don't show list in JSON format as it would break JSON output
		return
	case output.PlainFormat, output.TermFormat:
		// Show an empty line before the list for better separation
		termFormatter, err := formatter.NewTerminalFormatter(nil)
		if err != nil {
			log.Debug().Err(err).Msg("Failed to create formatter")
			return
		}

		// Add spacing before the list
		if len(scratches) > 0 {
			fmt.Println()
			fmt.Println()
		}

		if err := termFormatter.FormatList(scratches, global); err != nil {
			log.Debug().Err(err).Msg("Failed to format list")
			return
		}

		// Add spacing after the list before the success message
		if len(scratches) > 0 {
			fmt.Println()
		}
	}
}
