package cli

import (
	"os"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newCopyCmd creates and returns a new copy command
func newCopyCmd() *cobra.Command {
	return &cobra.Command{
		Use:     CopyUse,
		Aliases: []string{"cp"},
		Short:   CopyShort,
		Long:    CopyLong,
		Args:    cobra.MinimumNArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			global, _ := cmd.Flags().GetBool("global")

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg(ErrFailedToGetWorkingDir)
			}

			count, err := commands.CopyMultipleWithStoreManager(dir, global, args)
			if err != nil {
				log.Fatal().Err(err).Msg(ErrFailedToCopyScratch)
			}

			if count == 1 {
				log.Info().Msg(SuccessCopiedToClipboard)
			} else {
				log.Info().Msgf("Copied %d scratches to clipboard", count)
			}
		},
	}
}
