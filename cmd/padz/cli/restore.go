package cli

import (
	"fmt"
	"os"
	"time"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/utils"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newRestoreCmd creates and returns a new restore command
func newRestoreCmd() *cobra.Command {
	var all, global bool
	var projectFlag string
	var newerThanStr string

	cmd := &cobra.Command{
		Use:     "restore [id...]",
		Aliases: []string{"undelete", "recover"},
		Short:   "Restore soft-deleted scratches",
		Long: `Restore soft-deleted scratches back to active state.

Examples:
  padz restore d1               # Restore specific deleted scratch
  padz restore d1 d2 d3         # Restore multiple deleted scratches
  padz restore --newer-than 1h  # Restore scratches deleted less than 1 hour ago
  padz restore --all            # Restore all deleted scratches from all projects`,
		Args: cobra.MinimumNArgs(0),
		Run: func(cmd *cobra.Command, args []string) {
			// Format output
			format, formatErr := output.GetFormat(outputFormat)
			if formatErr != nil {
				log.Fatal().Err(formatErr).Msg("Failed to get output format")
			}

			// Parse duration if provided
			var newerThan time.Duration
			if newerThanStr != "" {
				var err error
				newerThan, err = utils.ParseDuration(newerThanStr)
				if err != nil {
					log.Fatal().Err(err).Msg("Invalid duration format")
				}
			}

			// Get current directory
			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current working directory")
			}

			var restoredTitles []string

			// If specific IDs are provided, restore those
			if len(args) > 0 {
				restoredTitles, err = commands.RestoreMultipleWithStoreManager(dir, global, args)
				if err != nil {
					handleTerminalError(err, format)
					return
				}
			} else {
				// Otherwise restore based on criteria
				restoredTitles, err = commands.RestoreWithStoreManager(dir, global, newerThan)
				if err != nil {
					handleTerminalError(err, format)
					return
				}
			}

			// Show list after command
			ShowListAfterCommandWithStoreManager(dir, global, all)

			// Success message
			var message string
			if len(restoredTitles) == 0 {
				message = "No deleted scratches to restore"
			} else if len(restoredTitles) == 1 {
				message = fmt.Sprintf("Successfully restored scratch: %s", restoredTitles[0])
			} else {
				message = fmt.Sprintf("Successfully restored %d scratches", len(restoredTitles))
			}

			// Show specific message for time-based restore
			if len(args) == 0 && newerThan > 0 && len(restoredTitles) > 0 {
				message = fmt.Sprintf("Successfully restored %d scratches deleted within %s", len(restoredTitles), utils.FormatDuration(newerThan))
			}

			handleTerminalSuccess(message, format)
		},
	}

	cmd.Flags().BoolVarP(&all, "all", "a", false, "Restore from all projects")
	cmd.Flags().BoolVarP(&global, "global", "g", false, "Restore from global scope")
	cmd.Flags().StringVarP(&projectFlag, "project", "p", "", "Restore from specific project")
	cmd.Flags().StringVar(&newerThanStr, "newer-than", "", "Restore only items deleted less than duration ago (e.g., 1h, 30m)")

	return cmd
}
