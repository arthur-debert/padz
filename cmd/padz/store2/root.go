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
	cmd.AddCommand(newDeleteCommand())
	cmd.AddCommand(newSearchCommand())
	cmd.AddCommand(newCopyCommand())

	return cmd
}

// Export individual commands for v2 migration
func NewCreateCommand() *cobra.Command {
	return newCreateCommand()
}

func NewViewCommand() *cobra.Command {
	return newViewCommand()
}

func NewListCommand() *cobra.Command {
	return newListCommand()
}

func NewDeleteCommand() *cobra.Command {
	return newDeleteCommand()
}

func NewSearchCommand() *cobra.Command {
	return newSearchCommand()
}

func NewCopyCommand() *cobra.Command {
	return newCopyCommand()
}
