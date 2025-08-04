/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"log"
	"os"
	"os/exec"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"strings"

	"github.com/spf13/cobra"
)

// newViewCmd creates and returns a new view command
func newViewCmd() *cobra.Command {
	return &cobra.Command{
	Use:   "view <index>",
	Short: "View a scratch",
	Long:  `View the content of a scratch identified by its index.`,
	Args:  cobra.ExactArgs(1),
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

		content, err := commands.View(s, all, global, proj, args[0])
		if err != nil {
			log.Fatal(err)
		}

		// Check if output is being piped
		info, _ := os.Stdout.Stat()
		if (info.Mode() & os.ModeCharDevice) == 0 {
			fmt.Print(content)
		} else {
			// Use a pager
			pager := os.Getenv("PAGER")
			if pager == "" {
				pager = "less"
			}
			c := exec.Command(pager)
			c.Stdin = strings.NewReader(content)
			c.Stdout = os.Stdout
			if err := c.Run(); err != nil {
				log.Fatal(err)
			}
		}
	},
	}
}

