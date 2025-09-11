/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/store"
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

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current working directory")
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

			// For now, skip search functionality - we'll update it in Phase 2
			if searchTerm != "" {
				fmt.Println("Search functionality with StoreManager is not yet implemented. Please use without search for now.")
				return
			}

			// Use the new StoreManager approach for listing
			result, err := commands.LsWithStoreManager(dir, global, all, mode)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to list scratches")
			}

			var scratches []store.Scratch
			var scopedScratches []store.ScopedScratch

			// Handle different result types
			switch r := result.(type) {
			case []store.Scratch:
				scratches = r
			case []store.ScopedScratch:
				scopedScratches = r
			default:
				log.Fatal().Msg("Unexpected result type from LsWithStoreManager")
			}

			// Format output
			format, err := output.GetFormat(outputFormat)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get output format")
			}

			// Check if we have any results
			if len(scratches) == 0 && len(scopedScratches) == 0 {
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

				// Handle scoped vs regular scratches
				if len(scopedScratches) > 0 {
					// For now, convert ScopedScratches to regular scratches for display
					// In Phase 3, we'll update the formatter to show scope information
					convertedScratches := make([]store.Scratch, len(scopedScratches))
					for i, scoped := range scopedScratches {
						convertedScratches[i] = *scoped.Scratch
					}
					if err := termFormatter.FormatList(convertedScratches, all); err != nil {
						log.Fatal().Err(err).Msg("Failed to format scoped list")
					}
					// Print scope information as a temporary solution
					fmt.Printf("\nShowing results from scopes: ")
					seenScopes := make(map[string]bool)
					for _, scoped := range scopedScratches {
						if !seenScopes[scoped.Scope] {
							fmt.Printf("%s ", scoped.Scope)
							seenScopes[scoped.Scope] = true
						}
					}
					fmt.Println()
				} else {
					if err := termFormatter.FormatList(scratches, all); err != nil {
						log.Fatal().Err(err).Msg("Failed to format list")
					}
				}
			} else {
				// JSON format uses the standard formatter
				outputFormatter := output.NewFormatter(format, nil)
				if len(scopedScratches) > 0 {
					// Convert to something JSON-serializable
					// For now, just output the scratches without scope info
					convertedScratches := make([]store.Scratch, len(scopedScratches))
					for i, scoped := range scopedScratches {
						convertedScratches[i] = *scoped.Scratch
					}
					if err := outputFormatter.FormatList(convertedScratches, all); err != nil {
						log.Fatal().Err(err).Msg("Failed to format scoped list as JSON")
					}
				} else {
					if err := outputFormatter.FormatList(scratches, all); err != nil {
						log.Fatal().Err(err).Msg("Failed to format list as JSON")
					}
				}
			}
		},
	}
}
