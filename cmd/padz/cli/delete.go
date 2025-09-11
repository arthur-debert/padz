/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newDeleteCmd creates and returns a new delete command
func newDeleteCmd() *cobra.Command {
	return &cobra.Command{
		Use:     DeleteUse,
		Aliases: []string{"rm", "d", "del"},
		Short:   DeleteShort,
		Long:    DeleteLong,
		Args:    cobra.MinimumNArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			all, _ := cmd.Flags().GetBool("all")
			global, _ := cmd.Flags().GetBool("global")

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

			// Run discovery before deleting
			if err := s.RunDiscoveryBeforeCommand(); err != nil {
				log.Warn().Err(err).Msg("Failed to run discovery")
			}

			// Delete multiple scratches
			deletedTitles, err := commands.DeleteMultiple(s, all, global, proj, args)

			// Format output
			format, formatErr := output.GetFormat(outputFormat)
			if formatErr != nil {
				log.Fatal().Err(formatErr).Msg("Failed to get output format")
			}

			if err != nil {
				handleTerminalError(err, format)
				return
			}

			// Show list in verbose mode (before success message)
			ShowListAfterCommand(s, all, global, proj)

			// Show success message with scratch titles
			var successMsg string
			if len(deletedTitles) == 0 {
				successMsg = "No scratches were deleted (already deleted or not found)"
			} else if len(deletedTitles) == 1 {
				successMsg = fmt.Sprintf("The padz \"%s\" has been deleted", deletedTitles[0])
			} else {
				successMsg = fmt.Sprintf("%d padz have been deleted", len(deletedTitles))
			}
			handleTerminalSuccess(successMsg, format)
		},
	}
}
