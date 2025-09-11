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
func ShowListAfterCommand(s *store.Store, all, global bool, project string) {
	if !IsVerboseMode() {
		return
	}

	// Get the list of scratches
	scratches := commands.Ls(s, all, global, project)

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

		if err := termFormatter.FormatList(scratches, all || global); err != nil {
			log.Debug().Err(err).Msg("Failed to format list")
			return
		}

		// Add spacing after the list before the success message
		if len(scratches) > 0 {
			fmt.Println()
		}
	}
}

// ShowListAfterCommandWithStoreManager displays the list of scratches after a command using StoreManager
func ShowListAfterCommandWithStoreManager(workingDir string, globalFlag bool, allFlag bool) {
	if !IsVerboseMode() {
		return
	}

	// Get the list of scratches using StoreManager - always use ListModeActive for verbose display
	result, err := commands.LsWithStoreManager(workingDir, globalFlag, allFlag, commands.ListModeActive)
	if err != nil {
		log.Debug().Err(err).Msg("Failed to get list of scratches")
		return
	}

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
		// Type assert based on the result type
		var scratches []store.Scratch

		switch v := result.(type) {
		case []store.Scratch:
			scratches = v
		case []store.ScopedScratch:
			// For scoped results, extract the embedded scratch data
			for _, scopedScratch := range v {
				if scopedScratch.Scratch != nil {
					scratches = append(scratches, *scopedScratch.Scratch)
				}
			}
		default:
			log.Debug().Msgf("Unexpected result type from LsWithStoreManager: %T", result)
			return
		}

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

		if err := termFormatter.FormatList(scratches, allFlag || globalFlag); err != nil {
			log.Debug().Err(err).Msg("Failed to format list")
			return
		}

		// Add spacing after the list before the success message
		if len(scratches) > 0 {
			fmt.Println()
		}
	}
}
