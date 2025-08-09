package cli

import (
	"os"
	"strings"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newCreateCmd creates and returns a new create command
func newCreateCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "create [words...]",
		Aliases: []string{"new", "n"},
		Short:   "Create a new scratch",
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

			s, err := store.NewStore()
			if err != nil {
				log.Fatal().Err(err).Msg(ErrFailedToInitStore)
			}

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg(ErrFailedToGetWorkingDir)
			}

			proj := "global"
			if !globalFlag {
				currentProj, err := project.GetCurrentProject(dir)
				if err != nil {
					log.Fatal().Err(err).Msg(ErrFailedToGetProject)
				}
				proj = currentProj
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
				// No --title flag, args become the title
				title = strings.Join(args, " ")
			}

			// If content was piped, use it
			if len(pipedContent) > 0 {
				if err := commands.CreateWithTitle(s, proj, pipedContent, title); err != nil {
					log.Fatal().Err(err).Msg(ErrFailedToCreateNote)
				}
			} else {
				// No piped content, use initial content if any
				if err := commands.CreateWithTitleAndContent(s, proj, title, initialContent); err != nil {
					log.Fatal().Err(err).Msg(ErrFailedToCreateNote)
				}
			}
		},
	}
}
