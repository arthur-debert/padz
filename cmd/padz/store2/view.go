package store2

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/store2"
	"github.com/spf13/cobra"
)

func newViewCommand() *cobra.Command {
	var global bool
	var all bool

	cmd := &cobra.Command{
		Use:   "view [id]",
		Short: "View a pad from the v2 store",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			idStr := args[0]

			// Handle flags - global flag overrides ID resolution
			if global {
				// Force explicit global ID format
				idStr = "global-" + idStr
			}

			// Detect current scope for implicit ID resolution
			dir, err := os.Getwd()
			if err != nil {
				return fmt.Errorf("failed to get current directory: %w", err)
			}

			currentScope, err := store2.DetectScope(dir)
			if err != nil {
				return fmt.Errorf("failed to detect scope: %w", err)
			}

			// Create dispatcher and get pad with ID resolution
			dispatcher := store2.NewDispatcher()
			pad, content, resolvedScope, err := dispatcher.GetPad(idStr, currentScope)
			if err != nil {
				return fmt.Errorf("failed to get pad: %w", err)
			}

			// Display with explicit ID format
			explicitID := store2.FormatExplicitID(resolvedScope, pad.UserID)
			fmt.Printf("=== Pad %s ===\n", explicitID)
			if pad.Title != "" {
				fmt.Printf("Title: %s\n", pad.Title)
			}
			fmt.Printf("Created: %s\n", pad.CreatedAt.Format("2006-01-02 15:04:05"))
			fmt.Printf("Size: %d bytes\n", pad.Size)
			if resolvedScope != currentScope {
				fmt.Printf("Scope: %s (resolved from current: %s)\n", resolvedScope, currentScope)
			} else {
				fmt.Printf("Scope: %s\n", resolvedScope)
			}
			fmt.Println("\n--- Content ---")
			fmt.Println(content)

			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "View pad from global scope")
	cmd.Flags().BoolVarP(&all, "all", "a", false, "View pad from any scope (default behavior)")

	return cmd
}
