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
	"os"

	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newLsCmd creates and returns a new list command
func newLsCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "list",
		Aliases: []string{"ls"},
		Short:   "Lists all scratches for the current project (ls)",
		Long: `Lists all scratches for the current project.
The output includes the index, the relative time of creation, and the title of the scratch.

You can search for specific scratches using the -s or --search flag:
  padz ls -s "search term"
  padz ls --search="regex pattern"

Search results are ranked by:
  1. Exact title matches
  2. Title matches (partial)
  3. Content matches
  4. Match length
  5. Original order`,
		Run: func(cmd *cobra.Command, args []string) {
			all, _ := cmd.Flags().GetBool("all")
			global, _ := cmd.Flags().GetBool("global")
			searchTerm, _ := cmd.Flags().GetString("search")
			showDeleted, _ := cmd.Flags().GetBool("deleted")
			includeDeleted, _ := cmd.Flags().GetBool("include-deleted")

			s, err := store.NewStore()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to initialize store")
			}

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current working directory")
			}

			proj, err := project.GetCurrentProject(dir)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current project")
			}

			// Run discovery before listing
			if err := s.RunDiscoveryBeforeCommand(); err != nil {
				log.Warn().Err(err).Msg("Failed to run discovery")
			}

			// Determine list mode based on flags
			var mode commands.ListMode
			if showDeleted {
				mode = commands.ListModeDeleted
			} else if includeDeleted || all {
				// --all flag also shows deleted items intermingled with active ones
				mode = commands.ListModeAll
			} else {
				mode = commands.ListModeActive
			}

			// If search term provided, use search functionality
			if searchTerm != "" {
				searchResults, err := commands.SearchWithIndicesMode(s, all, global, proj, searchTerm, mode)
				if err != nil {
					log.Fatal().Err(err).Msg("Failed to search")
				}

				// Format search results
				format, err := output.GetFormat(outputFormat)
				if err != nil {
					log.Fatal().Err(err).Msg("Failed to get output format")
				}

				if len(searchResults) == 0 && (format == output.PlainFormat || format == output.TermFormat) {
					fmt.Println("No scratches found matching your search.")
					return
				}

				// Use terminal formatter for both plain and term formats
				if format == output.PlainFormat || format == output.TermFormat {
					termFormatter, err := formatter.NewTerminalFormatter(nil)
					if err != nil {
						log.Fatal().Err(err).Msg("Failed to create terminal formatter")
					}
					if err := termFormatter.FormatSearchResults(searchResults, all); err != nil {
						log.Fatal().Err(err).Msg("Failed to format search results")
					}
				} else {
					// JSON format should output the ScratchWithIndex objects
					outputFormatter := output.NewFormatter(format, nil)
					if err := outputFormatter.FormatSearchResults(searchResults, all); err != nil {
						log.Fatal().Err(err).Msg("Failed to format search results")
					}
				}
				return
			}

			// Normal listing without search
			scratches := commands.LsWithMode(s, all, global, proj, mode)

			// Format output
			format, err := output.GetFormat(outputFormat)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get output format")
			}

			if len(scratches) == 0 {
				if format == output.PlainFormat || format == output.TermFormat {
					fmt.Println("Nothing here, create your first scratch with `padz create` or `padz help` for assistance.")
				}
				return
			}

			// Use terminal formatter for both plain and term formats
			// Terminal detection will automatically strip formatting when piped
			if format == output.PlainFormat || format == output.TermFormat {
				termFormatter, err := formatter.NewTerminalFormatter(nil)
				if err != nil {
					log.Fatal().Err(err).Msg("Failed to create terminal formatter")
				}
				if err := termFormatter.FormatList(scratches, all); err != nil {
					log.Fatal().Err(err).Msg("Failed to format list")
				}
			} else {
				// JSON format uses the standard formatter
				formatter := output.NewFormatter(format, nil)
				if err := formatter.FormatList(scratches, all); err != nil {
					log.Fatal().Err(err).Msg("Failed to format list")
				}
			}
		},
	}
}
