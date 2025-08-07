package cli

import (
	"github.com/rs/zerolog/log"
	"os"

	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/output"
)

// handleTerminalError handles error output with terminal formatting support
func handleTerminalError(err error, format output.Format) {
	if format == output.PlainFormat || format == output.TermFormat {
		// Use terminal formatter for both plain and term formats
		// Terminal detection will automatically strip formatting when piped
		termFormatter, termErr := formatter.NewTerminalFormatter(nil)
		if termErr != nil {
			log.Fatal().Err(termErr).Msg("Failed to create terminal formatter")
		}
		termFormatter.FormatError(err)
	} else {
		outputFormatter := output.NewFormatter(format, nil)
		if formatErr := outputFormatter.FormatError(err); formatErr != nil {
			log.Fatal().Err(formatErr).Msg("Failed to get output format")
		}
	}
	os.Exit(1)
}

// handleTerminalSuccess handles success output with terminal formatting support
func handleTerminalSuccess(message string, format output.Format) {
	if format == output.PlainFormat || format == output.TermFormat {
		// Use terminal formatter for both plain and term formats
		// Terminal detection will automatically strip formatting when piped
		termFormatter, err := formatter.NewTerminalFormatter(nil)
		if err != nil {
			log.Fatal().Err(err).Msg("Operation failed")
		}
		termFormatter.FormatSuccess(message)
	} else {
		outputFormatter := output.NewFormatter(format, nil)
		if err := outputFormatter.FormatSuccess(message); err != nil {
			log.Fatal().Err(err).Msg("Operation failed")
		}
	}
}
