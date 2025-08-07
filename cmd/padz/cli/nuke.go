/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"bufio"
	"fmt"
	"github.com/rs/zerolog/log"
	"os"
	"path/filepath"
	"strings"

	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"

	"github.com/spf13/cobra"
)

// newNukeCmd creates and returns a new nuke command
func newNukeCmd() *cobra.Command {
	return &cobra.Command{
		Use:   NukeUse,
		Short: NukeShort,
		Long:  NukeLong,
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

			// Format output
			format, err := output.GetFormat(outputFormat)
			if err != nil {
				log.Fatal().Err(err).Msg("Operation failed")
			}

			// For JSON format, we need to handle confirmation differently
			if format == output.JSONFormat {
				// For JSON, we would need a --yes flag to proceed without confirmation
				// For now, we'll just fail with an error
				outputFormatter := output.NewFormatter(format, nil)
				if err := outputFormatter.FormatError(fmt.Errorf("interactive confirmation not supported in JSON format")); err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}
				return
			}

			// First, get the count of pads to delete
			var count int
			var confirmMsg string

			// Count the pads that would be deleted
			if all {
				count = len(s.GetScratches())
			} else if proj == "" {
				// Count global pads
				for _, scratch := range s.GetScratches() {
					if scratch.Project == "global" {
						count++
					}
				}
			} else {
				// Count project pads
				for _, scratch := range s.GetScratches() {
					if scratch.Project == proj {
						count++
					}
				}
			}

			// Check if there are any pads to delete
			if count == 0 {
				handleTerminalSuccess(NukeNoPadsFound, format)
				return
			}

			// Prepare confirmation message based on scope
			if all {
				confirmMsg = fmt.Sprintf(NukeConfirmAll, count)
			} else if proj == "" {
				confirmMsg = fmt.Sprintf(NukeConfirmGlobal, count)
			} else {
				// For project scope, extract just the project name from the path
				projectName := filepath.Base(proj)
				confirmMsg = fmt.Sprintf(NukeConfirmProject, count, projectName)
			}

			// Show confirmation prompt
			if format == output.PlainFormat || format == output.TermFormat {
				// For warnings, we should use the warning style
				warningStyle, err := formatter.NewTerminalFormatter(os.Stderr)
				if err != nil {
					fmt.Fprint(os.Stderr, confirmMsg)
				} else {
					warningStyle.FormatWarning(strings.TrimSpace(confirmMsg))
					fmt.Fprint(os.Stderr, " ")
				}

				// Read user confirmation
				reader := bufio.NewReader(os.Stdin)
				response, err := reader.ReadString('\n')
				if err != nil {
					handleTerminalError(err, format)
				}

				response = strings.TrimSpace(strings.ToLower(response))
				if response != "y" && response != "yes" {
					handleTerminalSuccess(NukeCancelled, format)
					return
				}

				// Actually perform the nuke operation
				result, err := commands.Nuke(s, all, proj)
				if err != nil {
					handleTerminalError(err, format)
				}

				// Use the actual deleted count from the result
				var successMsg string
				if all {
					successMsg = fmt.Sprintf(NukeSuccessAll, result.DeletedCount)
				} else if result.Scope == "global" {
					successMsg = fmt.Sprintf(NukeSuccessGlobal, result.DeletedCount)
				} else {
					projectName := filepath.Base(result.ProjectName)
					successMsg = fmt.Sprintf(NukeSuccessProject, result.DeletedCount, projectName)
				}

				handleTerminalSuccess(successMsg, format)
			}
		},
	}
}
