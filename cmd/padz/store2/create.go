package store2

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/store2"
	"github.com/spf13/cobra"
)

func newCreateCommand() *cobra.Command {
	var title string
	var global bool

	cmd := &cobra.Command{
		Use:   "create [content]",
		Short: "Create a new pad in the v2 store",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			content := args[0]

			// Determine scope
			var scope string
			if global {
				scope = "global"
			} else {
				dir, err := os.Getwd()
				if err != nil {
					return fmt.Errorf("failed to get current directory: %w", err)
				}

				scope, err = store2.DetectScope(dir)
				if err != nil {
					return fmt.Errorf("failed to detect scope: %w", err)
				}
			}

			// Create dispatcher and create pad
			dispatcher := store2.NewDispatcher()
			pad, err := dispatcher.CreatePad(content, title, scope)
			if err != nil {
				return fmt.Errorf("failed to create pad: %w", err)
			}

			// Display with explicit ID format
			explicitID := store2.FormatExplicitID(scope, pad.UserID)
			fmt.Printf("Created pad %s\n", explicitID)
			return nil
		},
	}

	cmd.Flags().StringVarP(&title, "title", "t", "", "Title for the pad")
	cmd.Flags().BoolVarP(&global, "global", "g", false, "Create pad in global scope")

	return cmd
}
