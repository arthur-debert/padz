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

// peekCmd represents the peek command
var peekCmd = &cobra.Command{
	Use:   "peek [index]",
	Short: "Peek at a scratch",
	Long:  `Peek at the first and last lines of a scratch.`,
	Args:  cobra.ExactArgs(1),
	Run: func(cmd *cobra.Command, args []string) {
		all, _ := cmd.Flags().GetBool("all")
		global, _ := cmd.Flags().GetBool("global")
		lines, _ := cmd.Flags().GetInt("lines")

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

		content, err := commands.Peek(s, all, global, proj, args[0], lines)
		if err != nil {
			log.Fatal(err)
		}

		fmt.Print(content)
	},
}

