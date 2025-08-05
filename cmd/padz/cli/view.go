/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"log"
	"os"
	"os/exec"
	"github.com/arthur-debert/padz/cmd/padz/formatter"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"strings"

	"github.com/spf13/cobra"
)

// newViewCmd creates and returns a new view command
func newViewCmd() *cobra.Command {
	return &cobra.Command{
	Use:   ViewUse,
	Short: ViewShort,
	Long:  ViewLong,
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
		
		// Format output
		format, err := output.GetFormat(outputFormat)
		if err != nil {
			log.Fatal(err)
		}
		
		if format == output.JSONFormat {
			// JSON output goes directly to stdout
			outputFormatter := output.NewFormatter(format, nil)
			if err := outputFormatter.FormatString(content); err != nil {
				log.Fatal(err)
			}
		} else if format == output.TermFormat {
			// Check if output is being piped for term format
			info, _ := os.Stdout.Stat()
			if (info.Mode() & os.ModeCharDevice) == 0 {
				// Piped - use terminal formatter without pager
				termFormatter, err := formatter.NewTerminalFormatter(nil)
				if err != nil {
					log.Fatal(err)
				}
				if err := termFormatter.FormatContentView(content); err != nil {
					log.Fatal(err)
				}
			} else {
				// Use terminal formatter with pager
				var styledContent strings.Builder
				termFormatter, err := formatter.NewTerminalFormatter(&styledContent)
				if err != nil {
					log.Fatal(err)
				}
				
				// Render styled content
				if err := termFormatter.FormatContentView(content); err != nil {
					log.Fatal(err)
				}
				
				// Use pager with styled content
				pager := os.Getenv("PAGER")
				if pager == "" {
					pager = "less -R" // -R flag to handle ANSI colors
				}
				c := exec.Command("sh", "-c", pager)
				c.Stdin = strings.NewReader(styledContent.String())
				c.Stdout = os.Stdout
				if err := c.Run(); err != nil {
					log.Fatal(err)
				}
			}
		} else {
			// Plain format - check if output is being piped
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
		}
	},
	}
}

