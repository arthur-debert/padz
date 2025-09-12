package cli

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newCleanupCommand() *cobra.Command {
	var global bool
	var all bool
	var dryRun bool

	cmd := &cobra.Command{
		Use:   "cleanup",
		Short: "Clean up orphaned files and optimize storage",
		Long: `Clean up orphaned content files that no longer have corresponding metadata entries.
This command will:
- Remove content files that don't have metadata entries
- Optionally flush all soft-deleted pads permanently
- Display cleanup statistics`,
		RunE: func(cmd *cobra.Command, args []string) error {
			if all {
				// Cleanup all scopes
				return cleanupAllScopes(dryRun)
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

			return cleanupScope(scope, dryRun)
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Clean up global scope")
	cmd.Flags().BoolVarP(&all, "all", "a", false, "Clean up all scopes")
	cmd.Flags().BoolVar(&dryRun, "dry-run", false, "Show what would be cleaned up without making changes")

	return cmd
}

func cleanupScope(scope string, dryRun bool) error {
	// Get store path
	storePath, err := store.GetStorePath(scope)
	if err != nil {
		return fmt.Errorf("failed to get store path for scope %s: %w", scope, err)
	}

	// Create store
	storeInstance, err := store.NewStore(storePath)
	if err != nil {
		return fmt.Errorf("failed to create store for scope %s: %w", scope, err)
	}

	fmt.Printf("=== Cleaning up scope '%s' ===\n\n", scope)

	orphanedFiles, err := findOrphanedFiles(storeInstance, storePath)
	if err != nil {
		return fmt.Errorf("failed to find orphaned files: %w", err)
	}

	if len(orphanedFiles) == 0 {
		fmt.Println("No orphaned files found")
	} else {
		fmt.Printf("Found %d orphaned content files:\n", len(orphanedFiles))
		for _, file := range orphanedFiles {
			fmt.Printf("  - %s\n", file)
		}

		if !dryRun {
			// Remove orphaned files
			removed := 0
			for _, file := range orphanedFiles {
				if err := os.Remove(file); err != nil {
					fmt.Printf("  Warning: Failed to remove %s: %v\n", file, err)
				} else {
					removed++
				}
			}
			fmt.Printf("Removed %d orphaned files\n", removed)
		} else {
			fmt.Println("(dry run - files not actually removed)")
		}
	}

	// Show deleted pads that could be flushed
	deletedPads, err := storeInstance.ListDeleted()
	if err != nil {
		return fmt.Errorf("failed to list deleted pads: %w", err)
	}

	if len(deletedPads) > 0 {
		fmt.Printf("\n%d soft-deleted pads found (use 'padz flush --all' to permanently remove them)\n", len(deletedPads))
	}

	return nil
}

func cleanupAllScopes(dryRun bool) error {
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

	totalOrphaned := 0
	totalDeleted := 0
	scopes := 0

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		scope := entry.Name()
		scopes++

		// Get store path
		storePath, err := store.GetStorePath(scope)
		if err != nil {
			fmt.Printf("Warning: Failed to get store path for scope %s: %v\n", scope, err)
			continue
		}

		// Create store
		storeInstance, err := store.NewStore(storePath)
		if err != nil {
			fmt.Printf("Warning: Failed to create store for scope %s: %v\n", scope, err)
			continue
		}

		fmt.Printf("=== Cleaning up scope '%s' ===\n", scope)

		orphanedFiles, err := findOrphanedFiles(storeInstance, storePath)
		if err != nil {
			fmt.Printf("Warning: Failed to find orphaned files in scope %s: %v\n", scope, err)
			continue
		}

		if len(orphanedFiles) > 0 {
			fmt.Printf("Found %d orphaned files\n", len(orphanedFiles))
			totalOrphaned += len(orphanedFiles)

			if !dryRun {
				removed := 0
				for _, file := range orphanedFiles {
					if err := os.Remove(file); err != nil {
						fmt.Printf("  Warning: Failed to remove %s: %v\n", file, err)
					} else {
						removed++
					}
				}
				fmt.Printf("Removed %d orphaned files\n", removed)
			}
		} else {
			fmt.Println("No orphaned files found")
		}

		// Count deleted pads
		deletedPads, err := storeInstance.ListDeleted()
		if err == nil && len(deletedPads) > 0 {
			fmt.Printf("%d soft-deleted pads\n", len(deletedPads))
			totalDeleted += len(deletedPads)
		}

		fmt.Println()
	}

	fmt.Printf("=== Summary ===\n")
	fmt.Printf("Scopes processed: %d\n", scopes)
	if totalOrphaned > 0 {
		fmt.Printf("Total orphaned files: %d\n", totalOrphaned)
		if dryRun {
			fmt.Println("(dry run - files not actually removed)")
		}
	}
	if totalDeleted > 0 {
		fmt.Printf("Total soft-deleted pads: %d (use 'padz flush --all' to permanently remove)\n", totalDeleted)
	}

	return nil
}

func findOrphanedFiles(storeInstance *store.Store, storePath string) ([]string, error) {
	dataDir := filepath.Join(storePath, "data")

	// Check if data directory exists
	if _, err := os.Stat(dataDir); os.IsNotExist(err) {
		return nil, nil // No data directory means no files to clean up
	}

	// Get all content files
	entries, err := os.ReadDir(dataDir)
	if err != nil {
		return nil, fmt.Errorf("failed to read data directory: %w", err)
	}

	// Get all pads to build a set of valid IDs
	allPads, err := storeInstance.ListAll()
	if err != nil {
		return nil, fmt.Errorf("failed to list all pads: %w", err)
	}

	validIDs := make(map[string]bool)
	for _, pad := range allPads {
		validIDs[pad.ID] = true
	}

	// Find orphaned files
	var orphanedFiles []string
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		filename := entry.Name()
		fullPath := filepath.Join(dataDir, filename)

		// Check if this file has a corresponding pad
		if !validIDs[filename] {
			orphanedFiles = append(orphanedFiles, fullPath)
		}
	}

	return orphanedFiles, nil
}
