package cli

import (
	"fmt"
	"os"
	"strings"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newCreateCmd creates and returns a new create command
func newCreateCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "create [words...]",
		Aliases: []string{"new", "n", "c"},
		Short:   "Create a new scratch (new, n, c)",
		Long: `Create a new scratch in the current project or global scope.

If no content is piped, opens your default editor to write the scratch.
You can specify a custom title with --title or use arguments as the title.

Examples:
  padz create How to forget Greece     # Title: "How to forget Greece"
  padz create --title "My Title"       # Title: "My Title"
  padz create --title "My Title" Some content  # Title: "My Title", initial content: "Some content"`,
		Run: func(cmd *cobra.Command, args []string) {
			globalFlag, _ := cmd.Flags().GetBool("global")
			titleFlag, _ := cmd.Flags().GetString("title")

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg(ErrFailedToGetWorkingDir)
			}

			pipedContent := commands.ReadContentFromPipe()

			// Determine title and content based on flags and args
			var title string
			var initialContent []byte

			if titleFlag != "" {
				// If --title is provided, use it as title
				title = titleFlag
				if len(args) > 0 {
					// Args become initial content
					initialContent = []byte(strings.Join(args, " "))
				}
			} else if len(args) > 0 {
				// No --title flag, check for punctuation-based split
				fullText := strings.Join(args, " ")
				title, initialContent = splitTitleAndContent(fullText)
			}

			// If content was piped, use it
			if len(pipedContent) > 0 {
				if err := commands.CreateWithStoreManager(dir, globalFlag, pipedContent, title); err != nil {
					log.Fatal().Err(err).Msg(ErrFailedToCreateNote)
				}
			} else {
				// No piped content, use initial content if any
				if err := commands.CreateWithStoreManager(dir, globalFlag, initialContent, title); err != nil {
					log.Fatal().Err(err).Msg(ErrFailedToCreateNote)
				}
			}

			// Show list after command
			ShowListAfterCommandWithStoreManager(dir, globalFlag, false)

			// Show success message if not silent
			if !IsSilentMode() {
				format, err := output.GetFormat(outputFormat)
				if err != nil {
					log.Fatal().Err(err).Msg("Failed to get output format")
				}

				successMsg := "Scratch created successfully"
				if title != "" {
					successMsg = fmt.Sprintf("Scratch \"%s\" created successfully", title)
				}
				handleTerminalSuccess(successMsg, format)
			}
		},
	}
}

// splitTitleAndContent splits input text at the first sentence-ending punctuation
func splitTitleAndContent(text string) (string, []byte) {
	// Find first occurrence of . ! or ?
	punctMarks := []string{".", "!", "?"}
	earliestIndex := -1

	for _, mark := range punctMarks {
		if idx := strings.Index(text, mark); idx != -1 && (earliestIndex == -1 || idx < earliestIndex) {
			earliestIndex = idx
		}
	}

	if earliestIndex != -1 {
		// Split at punctuation
		title := strings.TrimSpace(text[:earliestIndex])
		content := strings.TrimSpace(text[earliestIndex+1:])

		if content != "" {
			return title, []byte(content)
		}
		// If no content after punctuation, use full text as title
		return text, nil
	}

	// No punctuation found, use entire text as title
	return text, nil
}
