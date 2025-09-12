package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newDeleteCommand() *cobra.Command {
	var global bool
	var all bool

	cmd := &cobra.Command{
		Use:   "delete [id]...",
		Short: "Delete one or more pads",
		Args:  cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			// Detect current scope for implicit ID resolution
			dir, err := os.Getwd()
			if err != nil {
				return fmt.Errorf("failed to get current directory: %w", err)
			}

			currentScope, err := store.DetectScope(dir)
			if err != nil {
				return fmt.Errorf("failed to detect scope: %w", err)
			}

			// Create dispatcher
			dispatcher := store.NewDispatcher()

			var deletedIDs []string
			var errors []error

			// Delete each pad
			for _, idStr := range args {
				// Handle global flag
				if global {
					// Force explicit global ID format
					idStr = "global-" + idStr
				}

				explicitID, err := dispatcher.DeletePad(idStr, currentScope)
				if err != nil {
					errors = append(errors, fmt.Errorf("failed to delete pad %s: %w", idStr, err))
				} else {
					deletedIDs = append(deletedIDs, explicitID)
				}
			}

			// Report results
			if len(deletedIDs) > 0 {
				if len(deletedIDs) == 1 {
					fmt.Printf("Deleted pad %s\n", deletedIDs[0])
				} else {
					fmt.Printf("Deleted %d pad(s): %s\n", len(deletedIDs), deletedIDs[0])
					for _, id := range deletedIDs[1:] {
						fmt.Printf("  %s\n", id)
					}
				}
			}

			// Report any errors
			if len(errors) > 0 {
				fmt.Printf("Failed to delete %d pad(s):\n", len(errors))
				for _, err := range errors {
					fmt.Printf("  %v\n", err)
				}
				return fmt.Errorf("some deletions failed")
			}

			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Delete pad from global scope")
	cmd.Flags().BoolVarP(&all, "all", "a", false, "Delete pad from any scope (default behavior)")

	return cmd
}
