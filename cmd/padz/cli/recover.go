package cli

import (
	"fmt"
	"os"
	"path/filepath"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/google/uuid"
	"github.com/spf13/cobra"
)

func newRecoverCommand() *cobra.Command {
	var dryRun bool

	cmd := &cobra.Command{
		Use:   "recover",
		Short: "Recover orphaned content files",
		Long: `Recover orphaned content files that have no corresponding metadata entries.
This can happen if metadata becomes corrupted or if files are manually added to the data directory.

The recover command will:
- Find content files without metadata entries
- Create new metadata entries for them
- Assign new user-friendly IDs
- Attempt to extract titles from content`,
		RunE: func(cmd *cobra.Command, args []string) error {
			return recoverAllScopes(dryRun)
		},
	}

	cmd.Flags().BoolVar(&dryRun, "dry-run", false, "Show what would be recovered without making changes")

	return cmd
}

func recoverAllScopes(dryRun bool) error {
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

	totalRecovered := 0

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		scope := entry.Name()
		recovered, err := recoverScope(scope, dryRun)
		if err != nil {
			fmt.Printf("Warning: Failed to recover scope %s: %v\n", scope, err)
			continue
		}

		totalRecovered += recovered
	}

	if totalRecovered == 0 {
		fmt.Println("No orphaned files found to recover")
	} else {
		if dryRun {
			fmt.Printf("Would recover %d orphaned files (dry run)\n", totalRecovered)
		} else {
			fmt.Printf("Successfully recovered %d orphaned files\n", totalRecovered)
		}
	}

	return nil
}

func recoverScope(scope string, dryRun bool) (int, error) {
	// Get store path
	storePath, err := store.GetStorePath(scope)
	if err != nil {
		return 0, fmt.Errorf("failed to get store path for scope %s: %w", scope, err)
	}

	dataDir := filepath.Join(storePath, "data")

	// Check if data directory exists
	if _, err := os.Stat(dataDir); os.IsNotExist(err) {
		return 0, nil // No data directory means no files to recover
	}

	// Create store
	storeInstance, err := store.NewStore(storePath)
	if err != nil {
		return 0, fmt.Errorf("failed to create store for scope %s: %w", scope, err)
	}

	// Get all existing pads to build a set of known IDs
	allPads, err := storeInstance.ListAll()
	if err != nil {
		return 0, fmt.Errorf("failed to list pads in scope %s: %w", scope, err)
	}

	knownIDs := make(map[string]bool)
	for _, pad := range allPads {
		knownIDs[pad.ID] = true
	}

	// Find orphaned files
	entries, err := os.ReadDir(dataDir)
	if err != nil {
		return 0, fmt.Errorf("failed to read data directory: %w", err)
	}

	var orphanedFiles []string
	for _, entry := range entries {
		if entry.IsDir() {
			continue
		}

		filename := entry.Name()

		// Check if this file has a corresponding pad
		if !knownIDs[filename] {
			// Validate that it looks like a UUID
			if _, err := uuid.Parse(filename); err == nil {
				orphanedFiles = append(orphanedFiles, filename)
			}
		}
	}

	if len(orphanedFiles) == 0 {
		return 0, nil
	}

	fmt.Printf("=== Recovering scope '%s' ===\n", scope)
	fmt.Printf("Found %d orphaned content files:\n", len(orphanedFiles))

	recovered := 0
	for _, fileID := range orphanedFiles {
		filePath := filepath.Join(dataDir, fileID)

		if dryRun {
			fmt.Printf("  - Would recover %s\n", fileID)
			recovered++
			continue
		}

		// Read the content
		content, err := os.ReadFile(filePath)
		if err != nil {
			fmt.Printf("  - Error reading %s: %v\n", fileID, err)
			continue
		}

		// Extract title from content - use the same function from open.go
		title := extractTitleFromContent(string(content))
		if title == "" {
			title = fmt.Sprintf("Recovered pad %s", fileID[:8])
		}

		// Create a new pad with the recovered content
		newPad, err := storeInstance.Create(string(content), title)
		if err != nil {
			fmt.Printf("  - Error recovering %s: %v\n", fileID, err)
			continue
		}

		_ = newPad // New pad created successfully

		fmt.Printf("  + Recovered %s: %s\n", fileID, title)
		recovered++
	}

	fmt.Printf("Recovered %d files in scope '%s'\n\n", recovered, scope)
	return recovered, nil
}
