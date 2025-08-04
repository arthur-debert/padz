package cli

import (
	"fmt"
	"os"

	"github.com/arthur-debert/padz/internal/version"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/logging"
	"github.com/arthur-debert/padz/pkg/project"
	"github.com/arthur-debert/padz/pkg/store"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

var (
	verbosity int
	verbose   bool
	debug     bool
)

// NewRootCmd creates and returns the root command
func NewRootCmd() *cobra.Command {
	rootCmd := &cobra.Command{
		Use:   "padz",
		Short: "A simple command-line note-taking tool",
		Long: `padz is a simple shell command to create, search, view, and edit quick notes.
It uses the user's default command-line editor for note creation and editing,
and focuses on streamlined content management.`,
		DisableAutoGenTag: true,
		CompletionOptions: cobra.CompletionOptions{
			DisableDefaultCmd: true,
		},
		PersistentPreRun: func(cmd *cobra.Command, args []string) {
			// Setup logging based on verbosity
			logging.SetupLogger(verbosity)
			log.Debug().Str("command", cmd.Name()).Msg("Command started")
		},
		Run: func(cmd *cobra.Command, args []string) {
			s, err := store.NewStore()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to initialize store")
			}

			dir, err := os.Getwd()
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get working directory")
			}

			proj, err := project.GetCurrentProject(dir)
			if err != nil {
				log.Fatal().Err(err).Msg("Failed to get current project")
			}

			content := commands.ReadContentFromPipe()
			if err := commands.Create(s, proj, content); err != nil {
				log.Fatal().Err(err).Msg("Failed to create note")
			}
		},
	}

	// Setup persistent flags
	rootCmd.PersistentFlags().CountVarP(&verbosity, "verbose", "v", "Increase verbosity (-v, -vv, -vvv)")
	rootCmd.PersistentFlags().BoolVar(&verbose, "verbose-old", false, "Enable verbose output")
	rootCmd.PersistentFlags().BoolVar(&debug, "debug", false, "Enable debug output")

	// Add version command
	rootCmd.AddCommand(&cobra.Command{
		Use:   "version",
		Short: "Print the version number",
		Long:  `Print the version number of padz`,
		Run: func(cmd *cobra.Command, args []string) {
			fmt.Printf("padz version %s (commit: %s, built: %s)\n", 
				version.Version, version.Commit, version.Date)
		},
	})

	// Add all the subcommands with their flags
	lsCmd.Flags().Bool("all", false, "Show scratches from all projects")
	lsCmd.Flags().Bool("global", false, "Show only global scratches")
	rootCmd.AddCommand(lsCmd)
	
	rootCmd.AddCommand(viewCmd)
	rootCmd.AddCommand(openCmd)
	rootCmd.AddCommand(deleteCmd)
	
	searchCmd.Flags().BoolP("all", "a", false, "Search in all projects")
	searchCmd.Flags().BoolP("global", "g", false, "Search in global scratches only")
	rootCmd.AddCommand(searchCmd)
	
	peekCmd.Flags().IntP("lines", "n", 3, "Number of lines to show from the beginning and end")
	peekCmd.Flags().Bool("all", false, "Show peek from all projects")
	peekCmd.Flags().Bool("global", false, "Show only global scratches")
	rootCmd.AddCommand(peekCmd)
	
	cleanupCmd.Flags().IntP("days", "d", 30, "Delete scratches older than this many days")
	rootCmd.AddCommand(cleanupCmd)

	return rootCmd
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() error {
	return NewRootCmd().Execute()
}