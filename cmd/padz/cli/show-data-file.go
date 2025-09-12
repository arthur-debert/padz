package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newShowDataFileCommand() *cobra.Command {
	var global bool
	var all bool

	cmd := &cobra.Command{
		Use:   "show-data-file",
		Short: "Show the path to the padz data directory",
		Long: `Show the file system path to where padz stores its data files.
This is useful for backup purposes or for advanced users who want to
access the underlying storage directly.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			if all {
				return showAllDataFiles()
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

			return showDataFileForScope(scope)
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Show global scope data file path")
	cmd.Flags().BoolVarP(&all, "all", "a", false, "Show data file paths for all scopes")

	return cmd
}

func showDataFileForScope(scope string) error {
	// Get store path
	storePath, err := store.GetStorePath(scope)
	if err != nil {
		return fmt.Errorf("failed to get store path: %w", err)
	}

	fmt.Printf("Data files for scope '%s':\n", scope)
	fmt.Printf("%s\n\n", storePath)

	// Show directory structure
	fmt.Println("Directory structure:")
	fmt.Printf("├── metadata.json\n")
	fmt.Printf("└── data/\n")

	// Check if directory exists and show stats
	if stat, err := os.Stat(storePath); err != nil {
		fmt.Printf("\nStatus: Directory does not exist\n")
		fmt.Printf("To create this scope, create your first pad with:\n")
		if scope == "global" {
			fmt.Printf("  padz create --global\n")
		} else {
			fmt.Printf("  padz create\n")
		}
	} else {
		fmt.Printf("\nStatus: Directory exists\n")
		fmt.Printf("Created: %s\n", stat.ModTime().Format("2006-01-02 15:04:05"))

		// Try to load store to get pad count
		if storeInstance, err := store.NewStore(storePath); err == nil {
			if pads, err := storeInstance.ListAll(); err == nil {
				active := 0
				deleted := 0
				pinned := 0
				for _, pad := range pads {
					if pad.IsDeleted {
						deleted++
					} else {
						active++
						if pad.IsPinned {
							pinned++
						}
					}
				}

				fmt.Printf("Total pads: %d\n", len(pads))
				if active > 0 {
					fmt.Printf("  Active: %d", active)
					if pinned > 0 {
						fmt.Printf(" (%d pinned)", pinned)
					}
					fmt.Println()
				}
				if deleted > 0 {
					fmt.Printf("  Deleted: %d\n", deleted)
				}
			}
		}
	}

	return nil
}

func showAllDataFiles() error {
	// Get base store directory
	baseDir, err := store.GetStorePath("")
	if err != nil {
		return fmt.Errorf("failed to get base store path: %w", err)
	}

	fmt.Printf("Padz base data directory:\n%s\n\n", baseDir)

	// Find all scope directories
	entries, err := os.ReadDir(baseDir)
	if err != nil {
		if os.IsNotExist(err) {
			fmt.Println("No data directory exists yet.")
			fmt.Println("Create your first pad to initialize storage:")
			fmt.Println("  padz create")
			return nil
		}
		return fmt.Errorf("failed to read store directory: %w", err)
	}

	if len(entries) == 0 {
		fmt.Println("Base directory exists but contains no scopes.")
		return nil
	}

	fmt.Println("Available scopes:")
	totalPads := 0

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		scope := entry.Name()
		scopePath, err := store.GetStorePath(scope)
		if err != nil {
			fmt.Printf("  ❌ %s (error: %v)\n", scope, err)
			continue
		}

		// Try to get pad count
		padCount := "?"
		if storeInstance, err := store.NewStore(scopePath); err == nil {
			if pads, err := storeInstance.ListAll(); err == nil {
				padCount = fmt.Sprintf("%d", len(pads))
				totalPads += len(pads)
			}
		}

		fmt.Printf("  📁 %s (%s pads)\n", scope, padCount)
		fmt.Printf("     %s\n", scopePath)
	}

	fmt.Printf("\nTotal: %d scope(s), %d pad(s)\n", len(entries), totalPads)

	// Show backup suggestion
	fmt.Printf("\nFor backup purposes, copy the entire directory:\n")
	fmt.Printf("  cp -r \"%s\" /path/to/backup/\n", baseDir)

	return nil
}
