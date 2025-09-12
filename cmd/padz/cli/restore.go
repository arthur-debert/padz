package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newRestoreCommand() *cobra.Command {
	var global bool

	cmd := &cobra.Command{
		Use:   "restore <ID>...",
		Short: "Restore soft-deleted pads",
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

			// Restore each pad
			restored := 0
			for _, idStr := range args {
				// Handle global flag
				if global {
					idStr = "global-" + idStr
				}

				// Parse and resolve ID
				parsedID, err := dispatcher.ParseID(idStr, currentScope)
				if err != nil {
					fmt.Printf("Error parsing ID %s: %v\n", idStr, err)
					continue
				}

				// Get the store for this scope
				storeInstance, err := dispatcher.GetStore(parsedID.Scope)
				if err != nil {
					fmt.Printf("Error getting store for scope %s: %v\n", parsedID.Scope, err)
					continue
				}

				// Restore the pad
				restoredPad, err := storeInstance.Restore(parsedID.UserID)
				if err != nil {
					fmt.Printf("Error restoring pad %s: %v\n", idStr, err)
					continue
				}

				fmt.Printf("Restored pad %d: %s\n", restoredPad.UserID, restoredPad.Title)
				restored++
			}

			if restored > 0 {
				fmt.Printf("\nSuccessfully restored %d pad(s)\n", restored)
			}

			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Restore from global scope")

	return cmd
}
