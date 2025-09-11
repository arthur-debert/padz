/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newCleanupCmd creates and returns a new cleanup command
func newCleanupCmd() *cobra.Command {
	return &cobra.Command{
		Use:     CleanupUse,
		Aliases: []string{"clean"},
		Short:   CleanupShort,
		Long:    CleanupLong,
		Run: func(cmd *cobra.Command, args []string) {
			days, _ := cmd.Flags().GetInt("days")

			// Get current directory
			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current working directory")
			}

			// Use StoreManager approach for cleanup
			deletedCount, err := commands.CleanupWithStoreManager(dir, days)

			// Format output
			format, formatErr := output.GetFormat(outputFormat)
			if formatErr != nil {
				log.Fatal().Err(formatErr).Msg("Failed to get output format")
			}

			if err != nil {
				handleTerminalError(err, format)
				return
			}

			// Show list after command
			ShowListAfterCommandWithStoreManager(dir, false, false)

			// Show success message with count of deleted items
			message := fmt.Sprintf("Cleanup completed successfully. Permanently deleted %d old scratches (older than %d days).", deletedCount, days)
			handleTerminalSuccess(message, format)
		},
	}
}
