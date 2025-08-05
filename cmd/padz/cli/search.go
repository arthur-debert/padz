/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"log"
	"os"

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

			if len(scratches) == 0 && (format == output.PlainFormat || format == output.TermFormat) {
				fmt.Println(SearchNoMatchesFound)
				return
			}

			// Use terminal formatter for both plain and term formats
			// Terminal detection will automatically strip formatting when piped
			if format == output.PlainFormat || format == output.TermFormat {
				termFormatter, err := formatter.NewTerminalFormatter(nil)
				if err != nil {
					log.Fatal(err)
				}
				if err := termFormatter.FormatList(scratches, all); err != nil {
					log.Fatal(err)
				}
			} else {
				// JSON format uses the standard formatter
				outputFormatter := output.NewFormatter(format, nil)
				if err := outputFormatter.FormatList(scratches, all); err != nil {
					log.Fatal(err)
				}
			}
		},
	}
}
