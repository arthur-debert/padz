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

// newFlushCmd creates and returns a new flush command
func newFlushCmd() *cobra.Command {
	var all, global bool
	var projectFlag string
	var olderThanStr string

	cmd := &cobra.Command{
		Use:     "flush [id]",
		Aliases: []string{"purge"},
		Short:   "Permanently delete soft-deleted scratches (hard delete)",
		Long: `Permanently delete soft-deleted scratches from disk.

Examples:
  padz flush                    # Flush all soft-deleted scratches
  padz flush d1                 # Flush specific deleted scratch
  padz flush --older-than 7d    # Flush deleted scratches older than 7 days
  padz flush --older-than 24h   # Flush deleted scratches older than 24 hours`,
		Run: func(cmd *cobra.Command, args []string) {
			// Parse duration if provided
			var olderThan time.Duration
			if olderThanStr != "" {
				var err error
				olderThan, err = utils.ParseDuration(olderThanStr)
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

			// If a specific ID is provided, flush that one
			if len(args) > 0 {
				err = commands.Flush(s, all, global, proj, args[0], 0)
			} else {
				// Otherwise flush based on criteria
				err = commands.Flush(s, all, global, proj, "", olderThan)
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

			// Success message
			var message string
			if len(args) > 0 {
				message = fmt.Sprintf("Successfully flushed scratch %s", args[0])
			} else if olderThan > 0 {
				message = fmt.Sprintf("Successfully flushed soft-deleted scratches older than %s", utils.FormatDuration(olderThan))
			} else {
				message = "Successfully flushed soft-deleted scratches"
			}

			handleTerminalSuccess(message, format)
		},
	}

	cmd.Flags().BoolVarP(&all, "all", "a", false, "Flush from all projects")
	cmd.Flags().BoolVarP(&global, "global", "g", false, "Flush from global scope")
	cmd.Flags().StringVarP(&projectFlag, "project", "p", "", "Flush from specific project")
	cmd.Flags().StringVar(&olderThanStr, "older-than", "", "Flush only items deleted more than duration ago (e.g., 7d, 24h)")

	return cmd
}
