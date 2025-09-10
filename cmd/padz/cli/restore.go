package cli

import (
	"fmt"
	"os"
	"time"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
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
		Use:     "restore [id]",
		Aliases: []string{"undelete", "recover"},
		Short:   "Restore soft-deleted scratches",
		Long: `Restore soft-deleted scratches back to active state.

Examples:
  padz restore d1               # Restore specific deleted scratch
  padz restore --newer-than 1h  # Restore scratches deleted less than 1 hour ago
  padz restore --all            # Restore all deleted scratches from all projects`,
		Run: func(cmd *cobra.Command, args []string) {
			// Parse duration if provided
			var newerThan time.Duration
			if newerThanStr != "" {
				var err error
				newerThan, err = utils.ParseDuration(newerThanStr)
				if err != nil {
					log.Fatal().Err(err).Msg("Invalid duration format")
				}
			}

			s, err := store.NewStore()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to initialize store")
			}

			// Get current directory
			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current working directory")
			}

			// Determine project
			proj := ""
			if projectFlag != "" {
				proj = projectFlag
			} else if !global && !all {
				currentProj, err := project.GetCurrentProject(dir)
				if err != nil {
					log.Fatal().Err(err).Msg("Failed to get current project")
				}
				proj = currentProj
			}

			// If a specific ID is provided, restore that one
			if len(args) > 0 {
				err = commands.Restore(s, all, global, proj, args[0], 0)
			} else {
				// Otherwise restore based on criteria
				err = commands.Restore(s, all, global, proj, "", newerThan)
			}

			// Format output
			format, formatErr := output.GetFormat(outputFormat)
			if formatErr != nil {
				log.Fatal().Err(formatErr).Msg("Failed to get output format")
			}

			if err != nil {
				handleTerminalError(err, format)
				return
			}

			// Show list after restore in verbose mode
			ShowListAfterCommand(s, all, global, proj)

			// Success message
			var message string
			if len(args) > 0 {
				message = fmt.Sprintf("Successfully restored scratch %s", args[0])
			} else if newerThan > 0 {
				message = fmt.Sprintf("Successfully restored scratches deleted within %s", utils.FormatDuration(newerThan))
			} else {
				message = "Successfully restored soft-deleted scratches"
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
