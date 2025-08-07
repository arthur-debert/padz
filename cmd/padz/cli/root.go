package cli

import (
	"github.com/arthur-debert/padz/internal/version"
	"github.com/arthur-debert/padz/pkg/logging"
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
		// Run ls command when no command is provided
		lsCmd := newLsCmd()
		lsCmd.Run(lsCmd, args)
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
	createCmd := newCreateCmd()
	createCmd.GroupID = "single"
	createCmd.Flags().BoolP("global", "g", false, "Create scratch in global scope")
	createCmd.Flags().StringP("title", "t", "", "Title for the scratch")
	rootCmd.AddCommand(createCmd)

	viewCmd := newViewCmd()
	viewCmd.GroupID = "single"
	viewCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	viewCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(viewCmd)

	openCmd := newOpenCmd()
	openCmd.GroupID = "single"
	openCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	rootCmd.AddCommand(openCmd)

	peekCmd := newPeekCmd()
	peekCmd.GroupID = "single"
	peekCmd.Flags().IntP("lines", "n", 3, FlagLinesDesc)
	peekCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	peekCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(peekCmd)

	deleteCmd := newDeleteCmd()
	deleteCmd.GroupID = "single"
	deleteCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	rootCmd.AddCommand(deleteCmd)

	pathCmd := newPathCmd()
	pathCmd.GroupID = "single"
	pathCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	rootCmd.AddCommand(pathCmd)

	// Multiple scratches commands
	lsCmd := newLsCmd()
	lsCmd.GroupID = "multiple"
	lsCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	lsCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
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
	nukeCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	rootCmd.AddCommand(nukeCmd)

	recoverCmd := newRecoverCmd()
	recoverCmd.GroupID = "multiple"
	rootCmd.AddCommand(recoverCmd)

	return rootCmd
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() error {
	return NewRootCmd().Execute()
}
