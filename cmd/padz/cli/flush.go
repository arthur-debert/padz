package cli

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newFlushCommand() *cobra.Command {
	var global bool
	var all bool
	var force bool

	cmd := &cobra.Command{
		Use:   "flush [ID]...",
		Short: "Permanently delete soft-deleted pads",
		Long:  "Permanently delete soft-deleted pads. Use --all to flush all deleted pads.",
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

			// Handle scope selection
			scope := currentScope
			if global {
				scope = "global"
			}

			if all {
				// Handle flush all
				return flushAllPads(scope, force)
			}

			// Require IDs if not using --all
			if len(args) == 0 {
				return fmt.Errorf("provide pad IDs to flush or use --all to flush all deleted pads")
			}

			// Create dispatcher
			dispatcher := store.NewDispatcher()

			// Confirm if not forced
			if !force {
				fmt.Printf("Warning: This will permanently delete %d pad(s). This action cannot be undone.\n", len(args))
				fmt.Print("Are you sure? (y/N): ")

				reader := bufio.NewReader(os.Stdin)
				response, _ := reader.ReadString('\n')
				response = strings.TrimSpace(strings.ToLower(response))

				if response != "y" && response != "yes" {
					fmt.Println("Operation cancelled")
					return nil
				}
			}

			// Flush each pad
			flushed := 0
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

				// Flush the pad
				flushedPad, err := storeInstance.Flush(parsedID.UserID)
				if err != nil {
					fmt.Printf("Error flushing pad %s: %v\n", idStr, err)
					continue
				}

				fmt.Printf("Permanently deleted pad %d: %s\n", flushedPad.UserID, flushedPad.Title)
				flushed++
			}

			if flushed > 0 {
				fmt.Printf("\nPermanently deleted %d pad(s)\n", flushed)
			}

			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Flush from global scope")
	cmd.Flags().BoolVarP(&all, "all", "a", false, "Flush all deleted pads")
	cmd.Flags().BoolVarP(&force, "force", "f", false, "Skip confirmation prompt")

	return cmd
}

func flushAllPads(scope string, force bool) error {
	// Get store path
	storePath, err := store.GetStorePath(scope)
	if err != nil {
		return fmt.Errorf("failed to get store path: %w", err)
	}

	// Create store
	storeInstance, err := store.NewStore(storePath)
	if err != nil {
		return fmt.Errorf("failed to create store: %w", err)
	}

	// Get count of deleted pads
	deletedPads, err := storeInstance.ListDeleted()
	if err != nil {
		return fmt.Errorf("failed to list deleted pads: %w", err)
	}

	if len(deletedPads) == 0 {
		fmt.Println("No deleted pads to flush")
		return nil
	}

	// Confirm if not forced
	if !force {
		fmt.Printf("Warning: This will permanently delete %d pad(s) from scope '%s'. This action cannot be undone.\n", len(deletedPads), scope)
		fmt.Print("Are you sure? (y/N): ")

		reader := bufio.NewReader(os.Stdin)
		response, _ := reader.ReadString('\n')
		response = strings.TrimSpace(strings.ToLower(response))

		if response != "y" && response != "yes" {
			fmt.Println("Operation cancelled")
			return nil
		}
	}

	// Flush all
	count, err := storeInstance.FlushAll()
	if err != nil {
		return fmt.Errorf("failed to flush all: %w", err)
	}

	fmt.Printf("Permanently deleted %d pad(s) from scope '%s'\n", count, scope)
	return nil
}
