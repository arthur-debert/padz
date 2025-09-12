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
		Use:   "delete [id]",
		Short: "Delete a pad",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			idStr := args[0]

			// Handle global flag
			if global {
				// Force explicit global ID format
				idStr = "global-" + idStr
			}

			// Detect current scope for implicit ID resolution
			dir, err := os.Getwd()
			if err != nil {
				return fmt.Errorf("failed to get current directory: %w", err)
			}

			currentScope, err := store.DetectScope(dir)
			if err != nil {
				return fmt.Errorf("failed to detect scope: %w", err)
			}

			// Create dispatcher and delete pad
			dispatcher := store.NewDispatcher()
			explicitID, err := dispatcher.DeletePad(idStr, currentScope)
			if err != nil {
				return fmt.Errorf("failed to delete pad: %w", err)
			}

			fmt.Printf("Deleted pad %s\n", explicitID)
			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Delete pad from global scope")
	cmd.Flags().BoolVarP(&all, "all", "a", false, "Delete pad from any scope (default behavior)")

	return cmd
}
