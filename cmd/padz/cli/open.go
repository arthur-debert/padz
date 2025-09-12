package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/arthur-debert/padz/pkg/editor"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

func newOpenCommand() *cobra.Command {
	var global bool

	cmd := &cobra.Command{
		Use:   "open <ID>...",
		Short: "Open pads in $EDITOR for editing",
		Args:  cobra.MinimumNArgs(1),
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

			// Open each pad
			opened := 0
			for _, idStr := range args {
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

				// Edit content in editor
				newContentBytes, err := editor.OpenInEditor([]byte(content))
				if err != nil {
					fmt.Printf("Error opening editor for pad %s: %v\n", idStr, err)
					continue
				}

				newContent := string(newContentBytes)

				// Extract title from content (first non-empty line)
				newTitle := extractTitleFromContent(newContent)
				if newTitle == "" {
					newTitle = pad.Title // Keep existing title if we can't extract one
				}

				// Check if content changed
				if newContent == content && newTitle == pad.Title {
					fmt.Printf("Pad %s: No changes made\n", idStr)
					continue
				}

				// Get the store for this pad's scope
				storeInstance, err := dispatcher.GetStore(resolvedScope)
				if err != nil {
					fmt.Printf("Error getting store for scope %s: %v\n", resolvedScope, err)
					continue
				}

				// Update the pad
				updatedPad, err := storeInstance.Update(pad.UserID, newContent, newTitle)
				if err != nil {
					fmt.Printf("Error updating pad %s: %v\n", idStr, err)
					continue
				}

				explicitID := store.FormatExplicitID(resolvedScope, updatedPad.UserID)
				fmt.Printf("Updated pad %s: %s\n", explicitID, updatedPad.Title)
				opened++
			}

			if opened > 0 {
				fmt.Printf("\nSuccessfully edited %d pad(s)\n", opened)
			}

			return nil
		},
	}

	cmd.Flags().BoolVarP(&global, "global", "g", false, "Open from global scope")

	return cmd
}

// extractTitleFromContent extracts a title from content (first non-empty line, up to 100 chars)
func extractTitleFromContent(content string) string {
	lines := strings.Split(content, "\n")
	for _, line := range lines {
		line = strings.TrimSpace(line)
		if line != "" {
			// Remove markdown header markers
			line = strings.TrimPrefix(line, "#")
			line = strings.TrimSpace(line)
			if len(line) > 100 {
				line = line[:100] + "..."
			}
			return line
		}
	}
	return ""
}
