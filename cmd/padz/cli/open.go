/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"os"

	"github.com/spf13/cobra"
)

// newOpenCmd creates and returns a new open command
func newOpenCmd() *cobra.Command {
	return &cobra.Command{
		Use:   OpenUse,
		Short: OpenShort,
		Long:  OpenLong,
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			all, _ := cmd.Flags().GetBool("all")

			s, err := store.NewStore()
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			proj, err := project.GetCurrentProject(dir)
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			err = commands.Open(s, all, proj, args[0])

			// Format output
			format, formatErr := output.GetFormat(outputFormat)
			if formatErr != nil {
				log.Fatal().Err(formatErr).Msg("Failed to get output format")
			}

			if err != nil {
				handleTerminalError(err, format)
			}

			handleTerminalSuccess(OpenSuccess, format)
		},
	}
}
