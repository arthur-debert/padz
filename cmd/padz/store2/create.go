package store2

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/store2"
	"github.com/spf13/cobra"
)

func newCreateCommand() *cobra.Command {
	var title string

	cmd := &cobra.Command{
		Use:   "create [content]",
		Short: "Create a new pad in the v2 store",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			content := args[0]

			// Detect scope
			dir, err := os.Getwd()
			if err != nil {
				return fmt.Errorf("failed to get current directory: %w", err)
			}

			scope, err := store2.DetectScope(dir)
			if err != nil {
				return fmt.Errorf("failed to detect scope: %w", err)
			}

			// Get store path
			storePath, err := store2.GetStorePath(scope)
			if err != nil {
				return fmt.Errorf("failed to get store path: %w", err)
			}

			// Create store
			store, err := store2.NewStore(storePath)
			if err != nil {
				return fmt.Errorf("failed to create store: %w", err)
			}

			// Create pad
			pad, err := store.Create(content, title)
			if err != nil {
				return fmt.Errorf("failed to create pad: %w", err)
			}

			fmt.Printf("Created pad %d in scope '%s'\n", pad.UserID, scope)
			return nil
		},
	}

	cmd.Flags().StringVarP(&title, "title", "t", "", "Title for the pad")

	return cmd
}
