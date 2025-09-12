package cli

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newPathCommand() *cobra.Command {
	var global bool

	cmd := &cobra.Command{
		Use:   "path [ID]",
		Short: "Show file system paths for pads or stores",
		Long: `Show the file system paths for pads or stores.
If no ID is provided, shows the current scope's store path.
If an ID is provided, shows the path to that specific pad's content file.`,
		Args: cobra.MaximumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			if len(args) == 0 {
				// Show store path
				return showStorePath(global)
			}

			// Show pad file path
			return showPadPath(args[0], global)
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Use global scope")

	return cmd
}

func showStorePath(global bool) error {
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

	storePath, err := store.GetStorePath(scope)
	if err != nil {
		return fmt.Errorf("failed to get store path: %w", err)
	}

	fmt.Printf("Store path for scope '%s':\n%s\n\n", scope, storePath)

	// Show directory structure
	fmt.Println("Directory structure:")
	fmt.Printf("├── metadata.json    (pad metadata)\n")
	fmt.Printf("└── data/           (pad content files)\n")

	// Show if directory exists and basic stats
	if stat, err := os.Stat(storePath); err != nil {
		fmt.Printf("\nStatus: Directory does not exist\n")
	} else {
		fmt.Printf("\nStatus: Directory exists\n")
		fmt.Printf("Created: %s\n", stat.ModTime().Format("2006-01-02 15:04:05"))

		// Count content files
		dataDir := filepath.Join(storePath, "data")
		if entries, err := os.ReadDir(dataDir); err == nil {
			fmt.Printf("Content files: %d\n", len(entries))
		}
	}

	return nil
}

func showPadPath(idStr string, global bool) error {
	// Detect current scope for implicit ID resolution
	dir, err := os.Getwd()
	if err != nil {
		return fmt.Errorf("failed to get current directory: %w", err)
	}

	currentScope, err := store.DetectScope(dir)
	if err != nil {
		return fmt.Errorf("failed to detect scope: %w", err)
	}

	// Handle global flag
	if global {
		idStr = "global-" + idStr
	}

	// Create dispatcher and get pad
	dispatcher := store.NewDispatcher()
	pad, _, resolvedScope, err := dispatcher.GetPad(idStr, currentScope)
	if err != nil {
		return fmt.Errorf("failed to get pad: %w", err)
	}

	// Get store path for this pad's scope
	storePath, err := store.GetStorePath(resolvedScope)
	if err != nil {
		return fmt.Errorf("failed to get store path: %w", err)
	}

	// Show pad information
	explicitID := store.FormatExplicitID(resolvedScope, pad.UserID)
	fmt.Printf("Pad %s: %s\n\n", explicitID, pad.Title)

	// Show paths
	contentPath := filepath.Join(storePath, "data", pad.ID)
	metadataPath := filepath.Join(storePath, "metadata.json")

	fmt.Printf("Content file path:\n%s\n\n", contentPath)
	fmt.Printf("Metadata file path:\n%s\n\n", metadataPath)

	// Show file status
	if stat, err := os.Stat(contentPath); err != nil {
		fmt.Printf("Content file status: MISSING\n")
	} else {
		fmt.Printf("Content file status: EXISTS\n")
		fmt.Printf("Size: %d bytes\n", stat.Size())
		fmt.Printf("Modified: %s\n", stat.ModTime().Format("2006-01-02 15:04:05"))
	}

	return nil
}
