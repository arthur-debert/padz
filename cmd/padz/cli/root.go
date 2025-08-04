package cli

import (
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
	outputFormat string
)

// NewRootCmd creates and returns the root command
func NewRootCmd() *cobra.Command {
	rootCmd := &cobra.Command{
		Use:   "padz",
		Short: "padz create scratch pads, draft files using $EDITOR.",
		Long: `padz create scratch pads, draft files using $EDITOR.

  $ padz                    # edit a new scratch in $EDITOR
  $ padz ls                 # Lists scratches with an index to be used in open, view, delete:
      1. 10 minutes ago My first scratch note
  $ padz view <index>       # views in shell
  $ padz search "<term>"    # search for scratches containing term`,
		DisableAutoGenTag: true,
		CompletionOptions: cobra.CompletionOptions{
			DisableDefaultCmd: true,
		},
		PersistentPreRun: func(cmd *cobra.Command, args []string) {
			// Setup logging based on verbosity
			logging.SetupLogger(verbosity)
			log.Debug().Str("command", cmd.Name()).Msg("Command started")
		},
	}

	// Setup persistent flags
	rootCmd.PersistentFlags().CountVarP(&verbosity, "verbose", "v", "Increase verbosity (-v, -vv, -vvv)")
	rootCmd.PersistentFlags().Lookup("verbose").Hidden = true
	rootCmd.PersistentFlags().StringVar(&outputFormat, "format", "plain", "Output format (plain, json, term)")

	// Add version flag
	var versionFlag bool
	rootCmd.Flags().BoolVar(&versionFlag, "version", false, "Print version information")
	rootCmd.Run = func(cmd *cobra.Command, args []string) {
		if versionFlag {
			cmd.Printf("padz version %s (commit: %s, built: %s)\n", 
				version.Version, version.Commit, version.Date)
			return
		}
		// Original run logic for creating a scratch
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
	}

	// Set up command groups
	rootCmd.AddGroup(&cobra.Group{
		ID:    "single",
		Title: "SINGLE SCRATCH:",
	})
	rootCmd.AddGroup(&cobra.Group{
		ID:    "multiple",
		Title: "SCRATCHES:",
	})

	// Single scratch commands
	viewCmd := newViewCmd()
	viewCmd.GroupID = "single"
	rootCmd.AddCommand(viewCmd)
	
	openCmd := newOpenCmd()
	openCmd.GroupID = "single"
	rootCmd.AddCommand(openCmd)
	
	peekCmd := newPeekCmd()
	peekCmd.GroupID = "single"
	peekCmd.Flags().IntP("lines", "n", 3, "Number of lines to show from the beginning and end")
	peekCmd.Flags().Bool("all", false, "Show peek from all projects")
	peekCmd.Flags().Bool("global", false, "Show only global scratches")
	rootCmd.AddCommand(peekCmd)
	
	deleteCmd := newDeleteCmd()
	deleteCmd.GroupID = "single"
	rootCmd.AddCommand(deleteCmd)
	
	// Multiple scratches commands
	lsCmd := newLsCmd()
	lsCmd.GroupID = "multiple"
	lsCmd.Flags().Bool("all", false, "Show scratches from all projects")
	lsCmd.Flags().Bool("global", false, "Show only global scratches")
	rootCmd.AddCommand(lsCmd)
	
	cleanupCmd := newCleanupCmd()
	cleanupCmd.GroupID = "multiple"
	cleanupCmd.Flags().IntP("days", "d", 30, "Delete scratches older than this many days")
	rootCmd.AddCommand(cleanupCmd)
	
	searchCmd := newSearchCmd()
	searchCmd.GroupID = "multiple"
	searchCmd.Flags().BoolP("all", "a", false, "Search in all projects")
	searchCmd.Flags().BoolP("global", "g", false, "Search in global scratches only")
	rootCmd.AddCommand(searchCmd)


	return rootCmd
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() error {
	return NewRootCmd().Execute()
}