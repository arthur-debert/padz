package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newPinCommand() *cobra.Command {
	var global bool

	cmd := &cobra.Command{
		Use:   "pin <ID>...",
		Short: "Pin pads for quick access",
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

			// Pin each pad
			pinned := 0
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

				// Pinned IDs can't be used to pin items
				if parsedID.Type == store.IDTypePinned {
					fmt.Printf("Error: ID %s is already a pinned ID\n", idStr)
					continue
				}

				// Deleted IDs can't be pinned
				if parsedID.Type == store.IDTypeDeleted {
					fmt.Printf("Error: Cannot pin deleted item %s\n", idStr)
					continue
				}

				// Get the store for this scope
				storeInstance, err := dispatcher.GetStore(parsedID.Scope)
				if err != nil {
					fmt.Printf("Error getting store for scope %s: %v\n", parsedID.Scope, err)
					continue
				}

				// Pin the pad
				pinnedPad, err := storeInstance.Pin(parsedID.UserID)
				if err != nil {
					fmt.Printf("Error pinning pad %s: %v\n", idStr, err)
					continue
				}

				fmt.Printf("Pinned pad %d: %s\n", pinnedPad.UserID, pinnedPad.Title)
				pinned++
			}

			if pinned > 0 {
				fmt.Printf("\nSuccessfully pinned %d pad(s)\n", pinned)
			}

			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Pin from global scope")

	return cmd
}

func newUnpinCommand() *cobra.Command {
	var global bool

	cmd := &cobra.Command{
		Use:   "unpin <ID>...",
		Short: "Unpin pads",
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

			// Unpin each pad
			unpinned := 0
			for _, idStr := range args {
				// Handle global flag
				if global {
					idStr = "global-" + idStr
				}

				// Parse and resolve ID - accept pinned IDs or regular IDs
				parsedID, err := dispatcher.ParseID(idStr, currentScope)
				if err != nil {
					fmt.Printf("Error parsing ID %s: %v\n", idStr, err)
					continue
				}

				// Deleted IDs can't be unpinned
				if parsedID.Type == store.IDTypeDeleted {
					fmt.Printf("Error: Cannot unpin deleted item %s\n", idStr)
					continue
				}

				// Get the store for this scope
				storeInstance, err := dispatcher.GetStore(parsedID.Scope)
				if err != nil {
					fmt.Printf("Error getting store for scope %s: %v\n", parsedID.Scope, err)
					continue
				}

				// Unpin the pad
				unpinnedPad, err := storeInstance.Unpin(parsedID.UserID)
				if err != nil {
					fmt.Printf("Error unpinning pad %s: %v\n", idStr, err)
					continue
				}

				fmt.Printf("Unpinned pad %d: %s\n", unpinnedPad.UserID, unpinnedPad.Title)
				unpinned++
			}

			if unpinned > 0 {
				fmt.Printf("\nSuccessfully unpinned %d pad(s)\n", unpinned)
			}

			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Unpin from global scope")

	return cmd
}
