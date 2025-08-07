/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"os"

	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newLsCmd creates and returns a new ls command
func newLsCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "ls",
		Short: "Lists all scratches for the current project",
		Long: `Lists all scratches for the current project.
The output includes the index, the relative time of creation, and the title of the scratch.`,
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

			scratches := commands.Ls(s, all, global, proj)

			// Format output
			format, err := output.GetFormat(outputFormat)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get output format")
			}

			if len(scratches) == 0 {
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
				if err := termFormatter.FormatList(scratches, all); err != nil {
					log.Fatal().Err(err).Msg("Failed to format list")
				}
			} else {
				// JSON format uses the standard formatter
				formatter := output.NewFormatter(format, nil)
				if err := formatter.FormatList(scratches, all); err != nil {
					log.Fatal().Err(err).Msg("Failed to format list")
				}
			}
		},
	}
}
