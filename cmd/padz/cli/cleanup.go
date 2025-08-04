/*
Copyright © 2025 YOUR NAME HERE <EMAIL ADDRESS>
*/
package cli

import (
	"fmt"
	"log"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/store"

	"github.com/spf13/cobra"
)

// cleanupCmd represents the cleanup command
var cleanupCmd = &cobra.Command{
	Use:   "cleanup",
	Short: "Cleanup old scratches",
	Long:  `Cleanup scratches older than a specified number of days.`,
	Run: func(cmd *cobra.Command, args []string) {
		days, _ := cmd.Flags().GetInt("days")

		s, err := store.NewStore()
		if err != nil {
			log.Fatal(err)
		}

		if err := commands.Cleanup(s, days); err != nil {
			log.Fatal(err)
		}

		fmt.Println("Cleanup complete.")
	},
}

