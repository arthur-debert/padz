package cli

import (
	"log"
	"os"

	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/output"
)

// handleTerminalError handles error output with terminal formatting support
func handleTerminalError(err error, format output.Format) {
	if format == output.TermFormat {
		termFormatter, termErr := formatter.NewTerminalFormatter(nil)
		if termErr != nil {
			log.Fatal(termErr)
		}
		termFormatter.FormatError(err)
	} else {
		outputFormatter := output.NewFormatter(format, nil)
		if formatErr := outputFormatter.FormatError(err); formatErr != nil {
			log.Fatal(formatErr)
		}
	}
	os.Exit(1)
}

// handleTerminalSuccess handles success output with terminal formatting support
func handleTerminalSuccess(message string, format output.Format) {
	if format == output.TermFormat {
		termFormatter, err := formatter.NewTerminalFormatter(nil)
		if err != nil {
			log.Fatal(err)
		}
		termFormatter.FormatSuccess(message)
	} else {
		outputFormatter := output.NewFormatter(format, nil)
		if err := outputFormatter.FormatSuccess(message); err != nil {
			log.Fatal(err)
		}
	}
}