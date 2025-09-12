package cli

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newExportCommand() *cobra.Command {
	var global bool
	var format string
	var output string

	cmd := &cobra.Command{
		Use:   "export <ID>... [output-path]",
		Short: "Export pads to files",
		Long: `Export one or more pads to files. If no output path is specified, files will be exported to the current directory.
Supported formats: txt (default), md (markdown)`,
		Args: cobra.MinimumNArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			// Extract IDs and optional output path
			ids := args
			outputPath := "."

			// If last argument looks like a path (not an ID), use it as output path
			if len(args) > 1 {
				lastArg := args[len(args)-1]
				// Check if it's a path-like string (contains slash or is a directory)
				if strings.Contains(lastArg, "/") || strings.Contains(lastArg, "\\") {
					stat, err := os.Stat(lastArg)
					if err == nil && stat.IsDir() {
						outputPath = lastArg
						ids = args[:len(args)-1]
					}
				}
			}

			// Use output flag if specified
			if output != "" {
				outputPath = output
			}

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

			// Export each pad
			exported := 0
			for _, idStr := range ids {
				// Handle global flag
				if global {
					idStr = "global-" + idStr
				}

				// Get pad content
				pad, content, resolvedScope, err := dispatcher.GetPad(idStr, currentScope)
				if err != nil {
					fmt.Printf("Error getting pad %s: %v\n", idStr, err)
					continue
				}

				// Generate filename
				filename := generateFilename(pad, resolvedScope, format)
				filePath := filepath.Join(outputPath, filename)

				// Ensure output directory exists
				if err := os.MkdirAll(filepath.Dir(filePath), 0755); err != nil {
					fmt.Printf("Error creating output directory for %s: %v\n", idStr, err)
					continue
				}

				// Export content
				exportContent := formatContent(content, pad, format)

				// Write to file
				if err := os.WriteFile(filePath, []byte(exportContent), 0644); err != nil {
					fmt.Printf("Error writing pad %s to file: %v\n", idStr, err)
					continue
				}

				fmt.Printf("Exported pad %s to %s\n", store.FormatExplicitID(resolvedScope, pad.UserID), filePath)
				exported++
			}

			if exported > 0 {
				fmt.Printf("\nSuccessfully exported %d pad(s)\n", exported)
			}

			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Export from global scope")
	cmd.Flags().StringVarP(&format, "format", "f", "txt", "Export format (txt, md)")
	cmd.Flags().StringVarP(&output, "output", "o", "", "Output directory path")

	return cmd
}

// generateFilename creates a filename for the exported pad
func generateFilename(pad *store.Pad, scope string, format string) string {
	// Start with title or ID
	name := pad.Title
	if name == "" {
		name = fmt.Sprintf("pad-%d", pad.UserID)
	}

	// Sanitize filename
	name = sanitizeFilename(name)

	// Add scope prefix if not default
	if scope != "global" {
		name = fmt.Sprintf("%s_%s", scope, name)
	}

	// Add extension
	ext := ".txt"
	if format == "md" {
		ext = ".md"
	}

	return name + ext
}

// sanitizeFilename removes characters that aren't safe for filenames
func sanitizeFilename(filename string) string {
	// Replace problematic characters with underscores
	replacer := strings.NewReplacer(
		"/", "_",
		"\\", "_",
		":", "_",
		"*", "_",
		"?", "_",
		"\"", "_",
		"<", "_",
		">", "_",
		"|", "_",
		"\n", "_",
		"\r", "_",
		"\t", "_",
	)

	sanitized := replacer.Replace(filename)

	// Trim spaces and limit length
	sanitized = strings.TrimSpace(sanitized)
	if len(sanitized) > 100 {
		sanitized = sanitized[:100]
	}

	// Ensure it's not empty
	if sanitized == "" {
		sanitized = "untitled"
	}

	return sanitized
}

// formatContent formats content according to the specified format
func formatContent(content string, pad *store.Pad, format string) string {
	switch format {
	case "md":
		// For markdown format, add title as header if available
		if pad.Title != "" {
			return fmt.Sprintf("# %s\n\n%s", pad.Title, content)
		}
		return content
	default: // txt
		// For text format, add title as first line if available
		if pad.Title != "" {
			return fmt.Sprintf("%s\n%s\n%s", pad.Title, strings.Repeat("=", len(pad.Title)), content)
		}
		return content
	}
}
