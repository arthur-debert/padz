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
	verbosity    int
	outputFormat string
)

// NewRootCmd creates and returns the root command
func NewRootCmd() *cobra.Command {
	rootCmd := &cobra.Command{
		Use:               RootUse,
		Short:             RootShort,
		Long:              RootLong,
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
	rootCmd.PersistentFlags().CountVarP(&verbosity, "verbose", "v", FlagVerboseDesc)
	rootCmd.PersistentFlags().Lookup("verbose").Hidden = true
	rootCmd.PersistentFlags().StringVarP(&outputFormat, "format", "f", "term", FlagFormatDesc)

	// Add version flag
	var versionFlag bool
	rootCmd.Flags().BoolVar(&versionFlag, "version", false, FlagVersionDesc)
	rootCmd.Run = func(cmd *cobra.Command, args []string) {
		if versionFlag {
			cmd.Printf(VersionFormat, version.Version, version.Commit, version.Date)
			return
		}
		// Original run logic for creating a scratch
		s, err := store.NewStore()
		if err != nil {
			log.Fatal().Err(err).Msg(ErrFailedToInitStore)
		}

		dir, err := os.Getwd()
		if err != nil {
			log.Fatal().Err(err).Msg(ErrFailedToGetWorkingDir)
		}

		proj, err := project.GetCurrentProject(dir)
		if err != nil {
			log.Fatal().Err(err).Msg(ErrFailedToGetProject)
		}

		content := commands.ReadContentFromPipe()
		if err := commands.Create(s, proj, content); err != nil {
			log.Fatal().Err(err).Msg(ErrFailedToCreateNote)
		}
	}

	// Set up command groups
	rootCmd.AddGroup(&cobra.Group{
		ID:    "single",
		Title: GroupSingleScratch,
	})
	rootCmd.AddGroup(&cobra.Group{
		ID:    "multiple",
		Title: GroupScratches,
	})

	// Single scratch commands
	viewCmd := newViewCmd()
	viewCmd.GroupID = "single"
	viewCmd.Flags().Bool("all", false, FlagAllDesc)
	viewCmd.Flags().Bool("global", false, FlagGlobalDesc)
	rootCmd.AddCommand(viewCmd)

	openCmd := newOpenCmd()
	openCmd.GroupID = "single"
	openCmd.Flags().Bool("all", false, FlagAllDesc)
	rootCmd.AddCommand(openCmd)

	peekCmd := newPeekCmd()
	peekCmd.GroupID = "single"
	peekCmd.Flags().IntP("lines", "n", 3, FlagLinesDesc)
	peekCmd.Flags().Bool("all", false, FlagAllDesc)
	peekCmd.Flags().Bool("global", false, FlagGlobalDesc)
	rootCmd.AddCommand(peekCmd)

	deleteCmd := newDeleteCmd()
	deleteCmd.GroupID = "single"
	deleteCmd.Flags().Bool("all", false, FlagAllDesc)
	rootCmd.AddCommand(deleteCmd)

	pathCmd := newPathCmd()
	pathCmd.GroupID = "single"
	pathCmd.Flags().Bool("all", false, FlagAllDesc)
	rootCmd.AddCommand(pathCmd)

	// Multiple scratches commands
	lsCmd := newLsCmd()
	lsCmd.GroupID = "multiple"
	lsCmd.Flags().Bool("all", false, FlagAllDesc)
	lsCmd.Flags().Bool("global", false, FlagGlobalDesc)
	rootCmd.AddCommand(lsCmd)

	cleanupCmd := newCleanupCmd()
	cleanupCmd.GroupID = "multiple"
	cleanupCmd.Flags().IntP("days", "d", 30, FlagDaysDesc)
	rootCmd.AddCommand(cleanupCmd)

	searchCmd := newSearchCmd()
	searchCmd.GroupID = "multiple"
	searchCmd.Flags().BoolP("all", "a", false, FlagAllDescSearch)
	searchCmd.Flags().BoolP("global", "g", false, FlagGlobalDescSearch)
	rootCmd.AddCommand(searchCmd)

	nukeCmd := newNukeCmd()
	nukeCmd.GroupID = "multiple"
	nukeCmd.Flags().Bool("all", false, FlagAllDesc)
	rootCmd.AddCommand(nukeCmd)

	return rootCmd
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() error {
	return NewRootCmd().Execute()
}
