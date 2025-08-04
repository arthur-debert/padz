/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"log"
	"os"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"

	"github.com/spf13/cobra"
)

// newLsCmd creates and returns a new ls command
func newLsCmd() *cobra.Command {
	return &cobra.Command{
	Use:   "ls",
	Short: "Lists all scratches for the current project",
	Long: `Lists all scratches for the current project.
The output includes the index, the relative time of creation, and the title of the scratch.`,
	Run: func(cmd *cobra.Command, args []string) {
		all, _ := cmd.Flags().GetBool("all")
		global, _ := cmd.Flags().GetBool("global")

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

		scratches := commands.Ls(s, all, global, proj)
		
		// Format output
		format, err := output.GetFormat(outputFormat)
		if err != nil {
			log.Fatal(err)
		}
		
		formatter := output.NewFormatter(format, nil)
		if len(scratches) == 0 && format == output.PlainFormat {
			fmt.Println("No scratches found.")
			return
		}
		
		if err := formatter.FormatList(scratches, all); err != nil {
			log.Fatal(err)
		}
	},
	}
}

