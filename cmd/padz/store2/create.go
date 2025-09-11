package store2

import (
	"fmt"

	"github.com/spf13/cobra"
)

func newCreateCommand() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "create [content]",
		Short: "Create a new pad in the v2 store",
		Args:  cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			content := args[0]
			fmt.Printf("Store2 Create: would create pad with content: %s\n", content)
			return nil
		},
	}

	return cmd
}
