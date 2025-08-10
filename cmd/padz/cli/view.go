/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"os"
	"os/exec"
	"strings"

	"github.com/spf13/cobra"
)

// newViewCmd creates and returns a new view command
func newViewCmd() *cobra.Command {
	return &cobra.Command{
		Use:   ViewUse,
		Short: ViewShort,
		Long:  ViewLong,
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			all, _ := cmd.Flags().GetBool("all")
			global, _ := cmd.Flags().GetBool("global")

			s, err := store.NewStore()
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			proj, err := project.GetCurrentProject(dir)
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			// Run discovery before viewing
			if err := s.RunDiscoveryBeforeCommand(); err != nil {
				log.Warn().Err(err).Msg("Failed to run discovery")
			}

			content, err := commands.View(s, all, global, proj, args[0])
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			// Format output
			format, err := output.GetFormat(outputFormat)
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			switch format {
			case output.JSONFormat:
				// JSON output goes directly to stdout
				outputFormatter := output.NewFormatter(format, nil)
				if err := outputFormatter.FormatString(content); err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}
			case output.PlainFormat, output.TermFormat:
				// Use terminal formatter for both plain and term formats
				// Terminal detection will automatically strip formatting when piped
				info, _ := os.Stdout.Stat()
				if (info.Mode() & os.ModeCharDevice) == 0 {
					// Piped - use terminal formatter without pager
					termFormatter, err := formatter.NewTerminalFormatter(nil)
					if err != nil {
						log.Fatal().Err(err).Msg("Operation failed")
					}
					if err := termFormatter.FormatContentView(content); err != nil {
						log.Fatal().Err(err).Msg("Operation failed")
					}
				} else {
					// Not piped - use terminal formatter with pager
					var styledContent strings.Builder
					termFormatter, err := formatter.NewTerminalFormatter(&styledContent)
					if err != nil {
						log.Fatal().Err(err).Msg("Operation failed")
					}

					// Render styled content
					if err := termFormatter.FormatContentView(content); err != nil {
						log.Fatal().Err(err).Msg("Operation failed")
					}

					// Use pager with styled content
					pager := os.Getenv("PAGER")
					if pager == "" {
						pager = "less -R" // -R flag to handle ANSI colors
					}
					c := exec.Command("sh", "-c", pager)
					c.Stdin = strings.NewReader(styledContent.String())
					c.Stdout = os.Stdout
					if err := c.Run(); err != nil {
						log.Fatal().Err(err).Msg("Operation failed")
					}
				}
			default:
				log.Fatal().Str("format", string(format)).Msg("Unsupported format")
			}
		},
	}
}
