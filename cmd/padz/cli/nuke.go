package cli

import (
	"bufio"
	"fmt"
	"os"
	"strings"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newNukeCommand() *cobra.Command {
	var global bool
	var all bool
	var force bool

	cmd := &cobra.Command{
		Use:   "nuke",
		Short: "DANGEROUS: Permanently delete ALL data",
		Long: `DANGEROUS: This command permanently deletes ALL pads and metadata.
This action is irreversible and will destroy all your data.
Use with extreme caution!

Examples:
  padz nuke                    # Delete all pads in current scope
  padz nuke --global           # Delete all pads in global scope
  padz nuke --all              # Delete all pads in ALL scopes
  padz nuke --force            # Skip confirmation (not recommended)`,
		RunE: func(cmd *cobra.Command, args []string) error {
			if all {
				return nukeAllScopes(force)
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

			return nukeScope(scope, force)
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Nuke global scope")
	cmd.Flags().BoolVarP(&all, "all", "a", false, "Nuke ALL scopes (extremely dangerous)")
	cmd.Flags().BoolVarP(&force, "force", "f", false, "Skip confirmation prompts (not recommended)")

	return cmd
}

func nukeScope(scope string, force bool) error {
	// Get store path
	storePath, err := store.GetStorePath(scope)
	if err != nil {
		return fmt.Errorf("failed to get store path: %w", err)
	}

	// Create store to check current contents
	storeInstance, err := store.NewStore(storePath)
	if err != nil {
		return fmt.Errorf("failed to create store: %w", err)
	}

	// Get current pad count
	allPads, err := storeInstance.ListAll()
	if err != nil {
		return fmt.Errorf("failed to list pads: %w", err)
	}

	if len(allPads) == 0 {
		fmt.Printf("Scope '%s' is already empty\n", scope)
		return nil
	}

	// Show what will be destroyed
	fmt.Printf("🚨 DANGER: This will permanently destroy ALL data in scope '%s'\n\n", scope)
	fmt.Printf("This will delete:\n")
	fmt.Printf("  - %d pads (including deleted ones)\n", len(allPads))
	fmt.Printf("  - All content files\n")
	fmt.Printf("  - All metadata\n")
	fmt.Printf("  - The entire scope directory\n\n")

	if !force {
		// Multiple confirmation steps
		fmt.Print("Do you really want to delete ALL data in this scope? (yes/no): ")
		if !confirmAction() {
			fmt.Println("Operation cancelled")
			return nil
		}

		fmt.Printf("This is your FINAL WARNING. Type the scope name '%s' to confirm: ", scope)
		reader := bufio.NewReader(os.Stdin)
		response, _ := reader.ReadString('\n')
		response = strings.TrimSpace(response)

		if response != scope {
			fmt.Println("Scope name mismatch. Operation cancelled")
			return nil
		}

		fmt.Print("Type 'DESTROY' in all caps to proceed: ")
		response, _ = reader.ReadString('\n')
		response = strings.TrimSpace(response)

		if response != "DESTROY" {
			fmt.Println("Confirmation failed. Operation cancelled")
			return nil
		}
	}

	// Remove the entire scope directory
	if err := os.RemoveAll(storePath); err != nil {
		return fmt.Errorf("failed to remove scope directory: %w", err)
	}

	fmt.Printf("💥 Scope '%s' has been completely destroyed (%d pads deleted)\n", scope, len(allPads))
	return nil
}

func nukeAllScopes(force bool) error {
	// Get base store directory
	baseDir, err := store.GetStorePath("")
	if err != nil {
		return fmt.Errorf("failed to get base store path: %w", err)
	}

	// Find all scope directories
	entries, err := os.ReadDir(baseDir)
	if err != nil {
		if os.IsNotExist(err) {
			fmt.Println("No store directories found")
			return nil
		}
		return fmt.Errorf("failed to read store directory: %w", err)
	}

	// Count total pads across all scopes
	totalPads := 0
	scopes := []string{}

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		scope := entry.Name()
		scopes = append(scopes, scope)

		// Get pad count for this scope
		storePath, err := store.GetStorePath(scope)
		if err != nil {
			continue
		}

		storeInstance, err := store.NewStore(storePath)
		if err != nil {
			continue
		}

		pads, err := storeInstance.ListAll()
		if err != nil {
			continue
		}

		totalPads += len(pads)
	}

	if totalPads == 0 {
		fmt.Println("All scopes are already empty")
		return nil
	}

	// Show what will be destroyed
	fmt.Printf("🚨🚨 EXTREME DANGER: This will permanently destroy ALL data in ALL scopes 🚨🚨\n\n")
	fmt.Printf("This will delete:\n")
	fmt.Printf("  - %d total pads across %d scopes\n", totalPads, len(scopes))
	fmt.Printf("  - All content files\n")
	fmt.Printf("  - All metadata\n")
	fmt.Printf("  - The entire padz store directory\n\n")
	fmt.Printf("Scopes to be destroyed:\n")
	for _, scope := range scopes {
		fmt.Printf("  - %s\n", scope)
	}
	fmt.Println()

	if !force {
		// Even more stringent confirmation for --all
		fmt.Print("Do you REALLY want to delete ALL data in ALL scopes? (yes/no): ")
		if !confirmAction() {
			fmt.Println("Operation cancelled")
			return nil
		}

		fmt.Print("This is IRREVERSIBLE. Type 'DELETE EVERYTHING' to confirm: ")
		reader := bufio.NewReader(os.Stdin)
		response, _ := reader.ReadString('\n')
		response = strings.TrimSpace(response)

		if response != "DELETE EVERYTHING" {
			fmt.Println("Confirmation failed. Operation cancelled")
			return nil
		}

		fmt.Print("Final confirmation. Type 'NUKE IT ALL' to proceed: ")
		response, _ = reader.ReadString('\n')
		response = strings.TrimSpace(response)

		if response != "NUKE IT ALL" {
			fmt.Println("Confirmation failed. Operation cancelled")
			return nil
		}
	}

	// Remove the entire store directory
	if err := os.RemoveAll(baseDir); err != nil {
		return fmt.Errorf("failed to remove store directory: %w", err)
	}

	fmt.Printf("💥💥 EVERYTHING DESTROYED: All %d scopes and %d pads have been permanently deleted 💥💥\n", len(scopes), totalPads)
	return nil
}

func confirmAction() bool {
	reader := bufio.NewReader(os.Stdin)
	response, _ := reader.ReadString('\n')
	response = strings.TrimSpace(strings.ToLower(response))
	return response == "yes" || response == "y"
}
