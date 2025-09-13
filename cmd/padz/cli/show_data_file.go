package cli

import (
	"encoding/json"
	"os"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"

	"github.com/spf13/cobra"
)

// newShowDataFileCmd creates and returns a new show-data-file command
func newShowDataFileCmd() *cobra.Command {
	return &cobra.Command{
		Use:   ShowDataFileUse,
		Short: ShowDataFileShort,
		Long:  ShowDataFileLong,
		Args:  cobra.NoArgs,
		Run: func(cmd *cobra.Command, args []string) {
			global, _ := cmd.Flags().GetBool("global")
			// Note: the global flag doesn't affect the data file location
			// All scratches are stored in the same place

			s, err := store.NewStoreWithScope(global)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to initialize store")
			}

			result, err := commands.ShowDataFile(s)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get data file path")
			}

			// Format output
			format, formatErr := output.GetFormat(outputFormat)
			if formatErr != nil {
				log.Fatal().Err(formatErr).Msg("Failed to get output format")
			}

			formatter := output.NewFormatter(format, nil)

			// For plain/term format, output the path directly
			if format == output.PlainFormat || format == output.TermFormat {
				if err := formatter.FormatString(result.Path); err != nil {
					log.Fatal().Err(err).Msg("Failed to format output")
				}
			} else {
				// For JSON, we need to encode the result manually since there's no FormatShowDataFile method
				if err := json.NewEncoder(os.Stdout).Encode(result); err != nil {
					log.Fatal().Err(err).Msg("Failed to format output")
				}
			}
		},
	}
}
