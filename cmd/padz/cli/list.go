package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newListCommand() *cobra.Command {
	var all bool
	var global bool
	var search string

	cmd := &cobra.Command{
		Use:   "list",
		Short: "List pads",
		RunE: func(cmd *cobra.Command, args []string) error {
			// Handle search flag
			if search != "" {
				// Use search command functionality
				if all || global {
					return searchAllScopes(search)
				}

				// Search current scope
				dir, err := os.Getwd()
				if err != nil {
					return fmt.Errorf("failed to get current directory: %w", err)
				}

				scope, err := store.DetectScope(dir)
				if err != nil {
					return fmt.Errorf("failed to detect scope: %w", err)
				}
				if global {
					scope = "global"
				}

				return searchScope(scope, search)
			}

			if all {
				// List from all scopes
				return listAllScopes()
			}

			// Determine scope
			var scope string
			if global {
				scope = "global"
			} else {
				dir, err := os.Getwd()
				if err != nil {
					return fmt.Errorf("failed to get current directory: %w", err)
				}

				scope, err = store.DetectScope(dir)
				if err != nil {
					return fmt.Errorf("failed to detect scope: %w", err)
				}
			}

			return listScope(scope)
		},
	}

	cmd.Flags().BoolVarP(&all, "all", "a", false, "List pads from all scopes")
	cmd.Flags().BoolVarP(&global, "global", "g", false, "List pads from global scope")
	cmd.Flags().StringVarP(&search, "search", "s", "", "Search for pads containing the given term")

	return cmd
}

func listScope(scope string) error {
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

	// List pads
	pads, err := storeInstance.List()
	if err != nil {
		return fmt.Errorf("failed to list pads: %w", err)
	}

	// Display
	fmt.Printf("=== Pads in scope '%s' ===\n\n", scope)
	if len(pads) == 0 {
		fmt.Println("No pads found")
		return nil
	}

	for _, pad := range pads {
		title := pad.Title
		if title == "" {
			title = "(untitled)"
		}
		explicitID := store.FormatExplicitID(scope, pad.UserID)
		fmt.Printf("%10s. %-50s %s\n", explicitID, title, pad.CreatedAt.Format("2006-01-02 15:04"))
	}

	return nil
}

func listAllScopes() error {
	dispatcher := store.NewDispatcher()

	results, errors := dispatcher.ListAllScopes()

	// Display results
	if len(results) == 0 {
		fmt.Println("No stores found")
		return nil
	}

	first := true
	for scope, pads := range results {
		if !first {
			fmt.Println() // Add spacing between scopes
		}
		first = false

		fmt.Printf("=== Pads in scope '%s' ===\n\n", scope)
		if len(pads) == 0 {
			fmt.Println("No pads found")
			continue
		}

		for _, pad := range pads {
			title := pad.Title
			if title == "" {
				title = "(untitled)"
			}
			explicitID := store.FormatExplicitID(scope, pad.UserID)
			fmt.Printf("%10s. %-50s %s\n", explicitID, title, pad.CreatedAt.Format("2006-01-02 15:04"))
		}
	}

	// Display errors as summary at the end
	if len(errors) > 0 {
		if len(results) > 0 {
			fmt.Println() // Add spacing
		}
		fmt.Printf("Encountered %d error(s) while listing scopes:\n", len(errors))
		for _, err := range errors {
			fmt.Printf("  - %v\n", err)
		}
	}

	return nil
}
