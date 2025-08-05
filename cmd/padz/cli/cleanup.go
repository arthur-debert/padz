/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/output"
	"github.com/arthur-debert/padz/pkg/store"
	"log"

	"github.com/spf13/cobra"
)

// newCleanupCmd creates and returns a new cleanup command
func newCleanupCmd() *cobra.Command {
	return &cobra.Command{
		Use:   CleanupUse,
		Short: CleanupShort,
		Long:  CleanupLong,
		Run: func(cmd *cobra.Command, args []string) {
			days, _ := cmd.Flags().GetInt("days")

			s, err := store.NewStore()
			if err != nil {
				log.Fatal(err)
			}

			err = commands.Cleanup(s, days)

			// Format output
			format, formatErr := output.GetFormat(outputFormat)
			if formatErr != nil {
				log.Fatal(formatErr)
			}

			if err != nil {
				handleTerminalError(err, format)
			}

			message := fmt.Sprintf(CleanupSuccessFormat, days)
			handleTerminalSuccess(message, format)
		},
	}
}
