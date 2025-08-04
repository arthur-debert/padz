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

// newSearchCmd creates and returns a new search command
func newSearchCmd() *cobra.Command {
	return &cobra.Command{
	Use:   SearchUse,
	Short: SearchShort,
	Long:  SearchLong,
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

		scratches, err := commands.Search(s, all, global, proj, args[0])
		if err != nil {
			log.Fatal(err)
		}
		
		// Format output
		format, err := output.GetFormat(outputFormat)
		if err != nil {
			log.Fatal(err)
		}
		
		formatter := output.NewFormatter(format, nil)
		if len(scratches) == 0 && format == output.PlainFormat {
			fmt.Println(SearchNoMatchesFound)
			return
		}
		
		if err := formatter.FormatList(scratches, all); err != nil {
			log.Fatal(err)
		}
	},
	}
}

