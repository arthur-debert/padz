package cli

import (
	"github.com/spf13/cobra"
)

// rootCmd represents the base command when called without any subcommands
var rootCmd = &cobra.Command{
	Use:   "padz",
	Short: "A simple command-line note-taking tool",
	Long: `padz is a simple shell command to create, search, view, and edit quick notes.
It uses multi-scope storage (project-scoped and global) for organized note management.`,
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main().
func Execute() error {
	return rootCmd.Execute()
}

func init() {
	// Add all commands to the root command
	rootCmd.AddCommand(newCreateCommand())
	rootCmd.AddCommand(newViewCommand())
	rootCmd.AddCommand(newListCommand())
	rootCmd.AddCommand(newDeleteCommand())
	rootCmd.AddCommand(newSearchCommand())
	rootCmd.AddCommand(newCopyCommand())

	// Add other commands that might be missing
	// TODO: Add other utility commands as needed
}

// Command constructors for the main CLI
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
