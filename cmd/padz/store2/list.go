package store2

import (
	"fmt"

	"github.com/spf13/cobra"
)

func newListCommand() *cobra.Command {
	var all bool

	cmd := &cobra.Command{
		Use:   "list",
		Short: "List pads from the v2 store",
		RunE: func(cmd *cobra.Command, args []string) error {
			if all {
				fmt.Println("Store2 List: would list pads from all scopes")
			} else {
				fmt.Println("Store2 List: would list pads from current scope")
			}
			return nil
		},
	}

	cmd.Flags().BoolVar(&all, "all", false, "List pads from all scopes")

	return cmd
}
