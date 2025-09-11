package store2

import (
	"github.com/spf13/cobra"
)

func NewStore2Command() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "store2",
		Short: "Experimental v2 store implementation",
		Long:  "POC implementation of the new multi-scope store design with simplified architecture",
	}

	cmd.AddCommand(newCreateCommand())
	cmd.AddCommand(newViewCommand())
	cmd.AddCommand(newListCommand())

	return cmd
}
