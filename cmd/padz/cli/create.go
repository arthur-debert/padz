package cli

import (
	"os"

	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

// newCreateCmd creates and returns a new create command
func newCreateCmd() *cobra.Command {
	return &cobra.Command{
		Use:     "create",
		Aliases: []string{"new", "n"},
		Short:   "Create a new scratch",
		Long: `Create a new scratch in the current project or global scope.

If no content is piped, opens your default editor to write the scratch.
You can specify a custom title and choose to create in global scope.`,
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

			content := commands.ReadContentFromPipe()

			if err := commands.CreateWithTitle(s, proj, content, titleFlag); err != nil {
				log.Fatal().Err(err).Msg(ErrFailedToCreateNote)
			}
		},
	}
}
