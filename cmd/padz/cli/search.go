/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"os"

	"github.com/spf13/cobra"
)

// newSearchCmd creates and returns a new search command
func newSearchCmd() *cobra.Command {
	return &cobra.Command{
		Use:   SearchUse,
		Short: SearchShort,
		Long:  SearchLong,
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

			// Run discovery before searching
			if err := s.RunDiscoveryBeforeCommand(); err != nil {
				log.Warn().Err(err).Msg("Failed to run discovery")
			}

			// Use SearchWithIndices to get results with correct positional indices
			searchResults, err := commands.SearchWithIndices(s, all, global, proj, args[0])
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			// Format output
			format, err := output.GetFormat(outputFormat)
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			if len(searchResults) == 0 && (format == output.PlainFormat || format == output.TermFormat) {
				fmt.Println(SearchNoMatchesFound)
				return
			}

			// Use terminal formatter for both plain and term formats
			// Terminal detection will automatically strip formatting when piped
			if format == output.PlainFormat || format == output.TermFormat {
				termFormatter, err := formatter.NewTerminalFormatter(nil)
				if err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}
				if err := termFormatter.FormatSearchResults(searchResults, all); err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}
			} else {
				// JSON format should output the ScratchWithIndex objects
				outputFormatter := output.NewFormatter(format, nil)
				if err := outputFormatter.FormatSearchResults(searchResults, all); err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}
			}
		},
	}
}
