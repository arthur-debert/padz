package commands

import (
	"fmt"
	"os"
	"path/filepath"
	"regexp"
	"strings"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
)

// Export exports scratches to files in the specified format
func Export(s *store.Store, all, global bool, project string, ids []string, format string) error {
	var scratches []store.Scratch

	if len(ids) == 0 {
		// Export all scratches
		scratches = Ls(s, all, global, project)
	} else {
		// Export specific scratches using centralized resolution
		resolvedScratches, err := ResolveMultipleIDs(s, all, global, project, ids)
		if err != nil {
			return err
		}
		// Convert pointers to values
		for _, scratch := range resolvedScratches {
			scratches = append(scratches, *scratch)
		}
	}

	if len(scratches) == 0 {
		return fmt.Errorf("no scratches to export")
	}

	// Create export directory
	dirName := fmt.Sprintf("padz-export-%s", time.Now().Format("2006-01-02-15-04"))
	if err := os.MkdirAll(dirName, 0755); err != nil {
		return fmt.Errorf("failed to create export directory: %w", err)
	}

	// Export each scratch
	exported := 0
	for i, scratch := range scratches {
		index := i + 1 // 1-based index
		filename := generateFilename(index, scratch.Title, format)
		filepath := filepath.Join(dirName, filename)

		content, err := readScratchFile(scratch.ID)
		if err != nil {
			return fmt.Errorf("failed to read scratch content: %w", err)
		}

		if err := os.WriteFile(filepath, content, 0644); err != nil {
			return fmt.Errorf("failed to write file %s: %w", filename, err)
		}
		exported++
	}

	fmt.Printf("Exported %d scratches to %s\n", exported, dirName)
	return nil
}

// ExportWithStoreManager exports scratches to files using StoreManager
func ExportWithStoreManager(workingDir string, globalFlag bool, ids []string, format string) error {
	sm := store.NewStoreManager()

	var allScratches []store.Scratch

	if len(ids) == 0 {
		// Export all scratches from the current store
		currentStore, _, err := sm.GetCurrentStore(workingDir, globalFlag)
		if err != nil {
			return fmt.Errorf("failed to get current store: %w", err)
		}

		// Get all active scratches
		scratches := currentStore.GetScratches()
		for _, s := range scratches {
			if !s.IsDeleted {
				allScratches = append(allScratches, s)
			}
		}
	} else {
		// Export specific scratches using aggregated resolution
		options := AggregateOptions{
			IncludeHeaders: false,
		}
		aggregatedContent, err := AggregateScratchContentsByIDsWithStoreManager(workingDir, globalFlag, ids, options)
		if err != nil {
			return err
		}

		// Convert aggregated content to scratches
		for _, scratch := range aggregatedContent.Scratches {
			if scratch != nil {
				allScratches = append(allScratches, *scratch)
			}
		}
	}

	if len(allScratches) == 0 {
		return fmt.Errorf("no scratches to export")
	}

	// Create export directory
	dirName := fmt.Sprintf("padz-export-%s", time.Now().Format("2006-01-02-15-04"))
	if err := os.MkdirAll(dirName, 0755); err != nil {
		return fmt.Errorf("failed to create export directory: %w", err)
	}

	// Export each scratch
	exported := 0
	for i, scratch := range allScratches {
		index := i + 1 // 1-based index
		filename := generateFilename(index, scratch.Title, format)
		filepath := filepath.Join(dirName, filename)

		content, err := readScratchFile(scratch.ID)
		if err != nil {
			return fmt.Errorf("failed to read scratch content: %w", err)
		}

		if err := os.WriteFile(filepath, content, 0644); err != nil {
			return fmt.Errorf("failed to write file %s: %w", filename, err)
		}
		exported++
	}

	fmt.Printf("Exported %d scratches to %s\n", exported, dirName)
	return nil
}

// generateFilename creates a filename from index and title
func generateFilename(index int, title string, format string) string {
	// Sanitize title
	sanitized := strings.ToLower(strings.TrimSpace(title))

	// Replace spaces with dashes
	sanitized = strings.ReplaceAll(sanitized, " ", "-")

	// Remove special characters, keep only alphanumeric and dashes
	reg := regexp.MustCompile(`[^a-z0-9\-]+`)
	sanitized = reg.ReplaceAllString(sanitized, "")

	// Truncate to 24 characters
	if len(sanitized) > 24 {
		sanitized = sanitized[:24]
	}

	// Remove trailing dashes
	sanitized = strings.Trim(sanitized, "-")

	// Determine extension
	ext := ".txt"
	if format == "markdown" || format == "md" {
		ext = ".md"
	}

	return fmt.Sprintf("%d-%s%s", index, sanitized, ext)
}
