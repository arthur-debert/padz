/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package main

import (
	"fmt"
	"log"
	"os"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"

	"github.com/dustin/go-humanize"
	"github.com/spf13/cobra"
)

// lsCmd represents the ls command
var lsCmd = &cobra.Command{
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

		for i, scratch := range scratches {
			if all {
				fmt.Printf("%d. %s %s %s\n", i+1, scratch.Project, humanize.Time(scratch.CreatedAt), scratch.Title)
			} else {
				fmt.Printf("%d. %s %s\n", i+1, humanize.Time(scratch.CreatedAt), scratch.Title)
			}
		}
	},
}

func init() {
	rootCmd.AddCommand(lsCmd)
	lsCmd.Flags().Bool("all", false, "Show scratches from all projects")
	lsCmd.Flags().Bool("global", false, "Show only global scratches")
}
