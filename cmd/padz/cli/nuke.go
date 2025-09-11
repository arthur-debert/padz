/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
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
			global, _ := cmd.Flags().GetBool("global")

			dir, err := os.Getwd()
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

			// First, get the count of pads to delete using StoreManager to count scratches
			// This is a simplified approach that gets an estimate
			var count int
			var confirmMsg string
			var scope string

			sm := store.NewStoreManager()

			if all {
				// Count from both global and current project stores
				globalStore, err := sm.GetGlobalStore()
				if err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}

				count = countActiveScratches(globalStore)
				scope = "all"

				// Try to count current project store too if different from global
				currentStore, _, err := sm.GetCurrentStore(dir, false)
				if err == nil && currentStore != globalStore {
					count += countActiveScratches(currentStore)
				}
			} else {
				// Count from specific store based on global flag
				currentStore, currentScope, err := sm.GetCurrentStore(dir, global)
				if err != nil {
					log.Fatal().Err(err).Msg("Operation failed")
				}

				count = countActiveScratches(currentStore)
				scope = currentScope
			}

			// Check if there are any pads to delete
			if count == 0 {
				handleTerminalSuccess(NukeNoPadsFound, format)
				return
			}

			// Prepare confirmation message based on scope
			if all {
				confirmMsg = fmt.Sprintf(NukeConfirmAll, count)
			} else if scope == "global" {
				confirmMsg = fmt.Sprintf(NukeConfirmGlobal, count)
			} else {
				// For project scope, extract project name from scope
				projectName := scope
				if strings.HasPrefix(scope, "project:") {
					projectName = strings.TrimPrefix(scope, "project:")
				}
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

				// Actually perform the nuke operation using StoreManager
				result, err := commands.NukeWithStoreManager(dir, global, all)
				if err != nil {
					handleTerminalError(err, format)
				}

				// Show list after command
				ShowListAfterCommandWithStoreManager(dir, global, all)

				// Use the actual deleted count from the result
				var successMsg string
				if all {
					successMsg = fmt.Sprintf(NukeSuccessAll, result.DeletedCount)
				} else if result.Scope == "global" {
					successMsg = fmt.Sprintf(NukeSuccessGlobal, result.DeletedCount)
				} else {
					successMsg = fmt.Sprintf(NukeSuccessProject, result.DeletedCount, result.ProjectName)
				}

				handleTerminalSuccess(successMsg, format)
			}
		},
	}
}

// countActiveScratches counts non-deleted scratches in a store
func countActiveScratches(s *store.Store) int {
	count := 0
	for _, scratch := range s.GetScratches() {
		if !scratch.IsDeleted {
			count++
		}
	}
	return count
}
