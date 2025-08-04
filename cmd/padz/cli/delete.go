/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"log"
	"os"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"

	"github.com/spf13/cobra"
)

// newDeleteCmd creates and returns a new delete command
func newDeleteCmd() *cobra.Command {
	return &cobra.Command{
	Use:   "delete <index>",
	Short: "Delete a scratch",
	Long:  `Delete a scratch identified by its index.`,
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
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

		if err := commands.Delete(s, proj, args[0]); err != nil {
			log.Fatal(err)
		}

		fmt.Println("Scratch deleted.")
	},
	}
}

