package store2

import (
	"fmt"
	"os"
	"strconv"

	"github.com/arthur-debert/padz/pkg/store2"
	"github.com/spf13/cobra"
)

func newViewCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "view [id]",
		Short: "View a pad from the v2 store",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			idStr := args[0]

			// Parse ID
			userID, err := strconv.Atoi(idStr)
			if err != nil {
				return fmt.Errorf("invalid ID format: %s", idStr)
			}

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

			// Get pad
			pad, content, err := store.Get(userID)
			if err != nil {
				return fmt.Errorf("failed to get pad: %w", err)
			}

			// Display
			fmt.Printf("=== Pad %d [%s] ===\n", pad.UserID, scope)
			if pad.Title != "" {
				fmt.Printf("Title: %s\n", pad.Title)
			}
			fmt.Printf("Created: %s\n", pad.CreatedAt.Format("2006-01-02 15:04:05"))
			fmt.Printf("Size: %d bytes\n", pad.Size)
			fmt.Println("\n--- Content ---")
			fmt.Println(content)

			return nil
		},
	}

	return cmd
}
