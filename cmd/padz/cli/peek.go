/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"bufio"
	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/rs/zerolog/log"
	"os"
	"strings"

	"github.com/spf13/cobra"
)

// newPeekCmd creates and returns a new peek command
func newPeekCmd() *cobra.Command {
	return &cobra.Command{
		Use:   PeekUse,
		Short: PeekShort,
		Long:  PeekLong,
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			global, _ := cmd.Flags().GetBool("global")
			lines, _ := cmd.Flags().GetInt("lines")

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			// Format output
			format, err := output.GetFormat(outputFormat)
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			if format == output.PlainFormat || format == output.TermFormat {
				// Use terminal formatter for both plain and term formats
				// Terminal detection will automatically strip formatting when piped
				content, err := commands.ViewWithStoreManager(dir, global, args[0])
				if err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}

				// Parse content into lines (excluding blank lines as per issue #12)
				scanner := bufio.NewScanner(strings.NewReader(content))
				var contentLines []string
				for scanner.Scan() {
					line := scanner.Text()
					if strings.TrimSpace(line) != "" { // Skip blank lines
						contentLines = append(contentLines, line)
					}
				}

				termFormatter, err := formatter.NewTerminalFormatter(nil)
				if err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}

				if len(contentLines) <= 2*lines {
					// Show full content
					if err := termFormatter.FormatContentView(strings.Join(contentLines, "\n")); err != nil {
						log.Fatal().Err(err).Msg("Operation failed")
					}
				} else {
					// Show peek format with start/end content and skipped count
					startLines := contentLines[:lines]
					endLines := contentLines[len(contentLines)-lines:]
					skippedLines := len(contentLines) - 2*lines

					startContent := strings.Join(startLines, "\n") + "\n"
					endContent := strings.Join(endLines, "\n")

					if err := termFormatter.FormatContentPeek(startContent, endContent, true, skippedLines); err != nil {
						log.Fatal().Err(err).Msg("Operation failed")
					}
				}
			} else {
				// For JSON format, use existing peek logic
				content, err := commands.PeekWithStoreManager(dir, global, args[0], lines)
				if err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}

				outputFormatter := output.NewFormatter(format, nil)
				if err := outputFormatter.FormatString(content); err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}
			}
		},
	}
}
