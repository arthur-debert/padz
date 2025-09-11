package store2

import (
	"fmt"

	"github.com/spf13/cobra"
)

func newViewCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "view [id]",
		Short: "View a pad from the v2 store",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			id := args[0]
			fmt.Printf("Store2 View: would view pad with ID: %s\n", id)
			return nil
		},
	}

	return cmd
}
