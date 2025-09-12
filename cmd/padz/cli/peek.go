package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newPeekCommand() *cobra.Command {
	var global bool
	var lines int

	cmd := &cobra.Command{
		Use:   "peek <ID>...",
		Short: "Preview pad contents without opening editor",
		Long: `Show a preview of pad contents without opening them in an editor.
By default shows the first 10 lines of each pad.`,
		Args: cobra.MinimumNArgs(1),
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

			// Preview each pad
			for i, idStr := range args {
				if i > 0 {
					fmt.Println() // Add spacing between pads
				}

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

				// Display header
				explicitID := store.FormatExplicitID(resolvedScope, pad.UserID)
				fmt.Printf("=== %s ===\n", explicitID)
				if pad.Title != "" {
					fmt.Printf("Title: %s\n", pad.Title)
				}
				fmt.Printf("Created: %s | Size: %d bytes",
					pad.CreatedAt.Format("2006-01-02 15:04"),
					pad.Size)
				if resolvedScope != currentScope {
					fmt.Printf(" | Scope: %s", resolvedScope)
				}
				if pad.IsPinned {
					fmt.Printf(" | 📌 Pinned")
				}
				if pad.IsDeleted {
					fmt.Printf(" | 🗑️ Deleted")
				}
				fmt.Println()
				fmt.Println()

				// Show content preview
				if content == "" {
					fmt.Println("(empty)")
				} else {
					showContentPreview(content, lines)
				}
			}

			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Peek from global scope")
	cmd.Flags().IntVarP(&lines, "lines", "n", 10, "Number of lines to show (0 for all)")

	return cmd
}

func showContentPreview(content string, maxLines int) {
	contentLines := strings.Split(content, "\n")

	// Determine how many lines to show
	linesToShow := len(contentLines)
	truncated := false
	if maxLines > 0 && len(contentLines) > maxLines {
		linesToShow = maxLines
		truncated = true
	}

	// Show the content
	for i := 0; i < linesToShow; i++ {
		line := contentLines[i]
		// Limit line length to avoid very long lines
		if len(line) > 120 {
			line = line[:120] + "..."
		}
		fmt.Println(line)
	}

	// Show truncation notice if needed
	if truncated {
		remaining := len(contentLines) - maxLines
		fmt.Printf("\n... (%d more lines)\n", remaining)
	}
}
