/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"log"
	"os"
	
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/spf13/cobra"
)

// newPathCmd creates and returns a new path command
func newPathCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "path <index>",
		Short: "Get the full path to a scratch",
		Long:  `Get the full path to a scratch file identified by its index.`,
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

			result, err := commands.Path(s, proj, args[0])
			
			// Format output
			format, formatErr := output.GetFormat(outputFormat)
			if formatErr != nil {
				log.Fatal(formatErr)
			}
			
			formatter := output.NewFormatter(format, nil)
			
			if err != nil {
				if err := formatter.FormatError(err); err != nil {
					log.Fatal(err)
				}
				os.Exit(1)
			}
			
			// For path command, output the path directly in plain/term mode
			if format == output.PlainFormat || format == output.TermFormat {
				if err := formatter.FormatString(result.Path); err != nil {
					log.Fatal(err)
				}
			} else {
				// For JSON, output the structured result
				if err := formatter.FormatPath(result); err != nil {
					log.Fatal(err)
				}
			}
		},
	}
}