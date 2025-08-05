/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"log"
	"os"

	"github.com/spf13/cobra"
)

// newDeleteCmd creates and returns a new delete command
func newDeleteCmd() *cobra.Command {
	return &cobra.Command{
		Use:   DeleteUse,
		Short: DeleteShort,
		Long:  DeleteLong,
		Args:  cobra.ExactArgs(1),
		Run: func(cmd *cobra.Command, args []string) {
			all, _ := cmd.Flags().GetBool("all")

			s, err := store.NewStore()
			if err != nil {
				log.Fatal(err)
			}

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal(err)
			}

			proj, err := project.GetCurrentProject(dir)
			if err != nil {
				log.Fatal(err)
			}

			err = commands.Delete(s, all, proj, args[0])

			// Format output
			format, formatErr := output.GetFormat(outputFormat)
			if formatErr != nil {
				log.Fatal(formatErr)
			}

			if err != nil {
				handleTerminalError(err, format)
			}

			handleTerminalSuccess(DeleteSuccess, format)
		},
	}
}
