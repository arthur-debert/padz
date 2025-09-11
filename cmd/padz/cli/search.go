package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newSearchCmd creates a new search command
func newSearchCmd() *cobra.Command {
	var all, global bool

	cmd := &cobra.Command{
		Use:   "search [term]",
		Short: "Search for scratches containing the given term",
		Long: `Search through scratch titles and content.

The search term can be provided as a single quoted argument or as multiple arguments
that will be joined with spaces. All text after 'search' is treated as the search term.

Examples:
  padz search "my term"
  padz search my term
  padz search This is a complex term

You can also use regular expressions:
  padz search "func.*main"
  padz search TODO|FIXME`,
		Args: cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			// Join all arguments as the search term
			searchTerm := strings.Join(args, " ")

			log.Debug().
				Str("term", searchTerm).
				Bool("all", all).
				Bool("global", global).
				Msg("Searching scratches")

			// Get current directory
			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current working directory")
			}

			// Use StoreManager approach for searching
			results, err := commands.SearchWithIndicesWithStoreManager(dir, global, searchTerm, all)
			if err != nil {
				log.Error().Err(err).Msg("Failed to search scratches")
				return err
			}

			// Format search results
			format, err := output.GetFormat(outputFormat)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get output format")
			}

			if len(results) == 0 && (format == output.PlainFormat || format == output.TermFormat) {
				fmt.Println("No scratches found matching your search.")
				return nil
			}

			// Use terminal formatter for both plain and term formats
			if format == output.PlainFormat || format == output.TermFormat {
				termFormatter, err := formatter.NewTerminalFormatter(nil)
				if err != nil {
					log.Fatal().Err(err).Msg("Failed to create terminal formatter")
				}
				if err := termFormatter.FormatSearchResults(results, all); err != nil {
					log.Fatal().Err(err).Msg("Failed to format search results")
				}
			} else {
				// JSON format should output the ScratchWithIndex objects
				outputFormatter := output.NewFormatter(format, nil)
				if err := outputFormatter.FormatSearchResults(results, all); err != nil {
					log.Fatal().Err(err).Msg("Failed to format search results")
				}
			}
			return nil
		},
	}

	return cmd
}
