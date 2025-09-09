package cli

import (
	"os"
	"strconv"
	"strings"

	"github.com/arthur-debert/padz/internal/version"
	"github.com/arthur-debert/padz/pkg/config"
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
	}

	// Setup persistent flags
	rootCmd.PersistentFlags().CountVarP(&verbosity, "verbose", "v", FlagVerboseDesc)
	rootCmd.PersistentFlags().Lookup("verbose").Hidden = true
	rootCmd.PersistentFlags().StringVarP(&outputFormat, "format", "f", "term", FlagFormatDesc)

	// Add version flag
	var versionFlag bool
	rootCmd.Flags().BoolVar(&versionFlag, "version", false, FlagVersionDesc)

	// Add hidden search flag to allow naked -s invocation
	var searchFlag string
	rootCmd.Flags().StringVarP(&searchFlag, "search", "s", "", "Search for scratches (redirects to ls -s)")
	rootCmd.Flags().Lookup("search").Hidden = true

	// Set PersistentPreRun for logging
	rootCmd.PersistentPreRun = func(cmd *cobra.Command, args []string) {
		// Setup logging based on verbosity
		logging.SetupLogger(verbosity)
		log.Debug().Str("command", cmd.Name()).Msg("Command started")
	}

	// Handle version flag in Run function
	rootCmd.Run = func(cmd *cobra.Command, args []string) {
		if versionFlag {
			cmd.Printf(VersionFormat, version.Version, version.Commit, version.Date)
			return
		}
		// This should not be reached normally due to our Execute() logic
		_ = cmd.Help()
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
	openCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
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
	deleteCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(deleteCmd)

	pathCmd := newPathCmd()
	pathCmd.GroupID = "single"
	pathCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	pathCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(pathCmd)

	copyCmd := newCopyCmd()
	copyCmd.GroupID = "single"
	copyCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	copyCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(copyCmd)

	pinCmd := newPinCmd()
	pinCmd.GroupID = "single"
	pinCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	pinCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(pinCmd)

	unpinCmd := newUnpinCmd()
	unpinCmd.GroupID = "single"
	unpinCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	unpinCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(unpinCmd)

	// Multiple scratches commands
	lsCmd := newLsCmd()
	lsCmd.GroupID = "multiple"
	lsCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	lsCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	lsCmd.Flags().StringP("search", "s", "", "Search for scratches containing the given term")
	rootCmd.AddCommand(lsCmd)

	cleanupCmd := newCleanupCmd()
	cleanupCmd.GroupID = "multiple"
	cleanupCmd.Flags().IntP("days", "d", 30, FlagDaysDesc)
	rootCmd.AddCommand(cleanupCmd)

	nukeCmd := newNukeCmd()
	nukeCmd.GroupID = "multiple"
	nukeCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	rootCmd.AddCommand(nukeCmd)

	recoverCmd := newRecoverCmd()
	recoverCmd.GroupID = "multiple"
	rootCmd.AddCommand(recoverCmd)

	exportCmd := newExportCmd()
	exportCmd.GroupID = "multiple"
	exportCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	exportCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(exportCmd)

	// Utility commands (not grouped)
	showDataFileCmd := newShowDataFileCmd()
	showDataFileCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(showDataFileCmd)

	return rootCmd
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() error {
	args := os.Args[1:]

	// Handle version flag specially
	for _, arg := range args {
		if arg == "--version" {
			rootCmd := NewRootCmd()
			rootCmd.SetArgs(args)
			return rootCmd.Execute()
		}
	}

	// Determine which command to run
	if shouldRunLs(args) {
		// Run ls command (handles: no args, or only search flags)
		os.Args = append([]string{os.Args[0], "ls"}, args...)
	} else if shouldRunViewOrOpen(args) {
		// Run view or open command (handles: single integer arg)
		cmd := config.NakedIntCommand // "view" or "open"
		os.Args = append([]string{os.Args[0], cmd}, args...)
	} else if shouldRunCreate(args) {
		// Run create command (handles: quoted strings or multiple args)
		os.Args = append([]string{os.Args[0], "create"}, args...)
	}

	return NewRootCmd().Execute()
}

// shouldRunLs determines if the arguments indicate an ls command
func shouldRunLs(args []string) bool {
	if len(args) == 0 {
		return true // No args = ls
	}

	// Check if first arg is a flag (starts with -)
	// This allows: padz -s "term", padz --search="term", padz -a, etc.
	if strings.HasPrefix(args[0], "-") {
		return true
	}

	return false
}

// shouldRunViewOrOpen determines if the arguments indicate a view/open command
func shouldRunViewOrOpen(args []string) bool {
	if len(args) != 1 {
		return false
	}

	// Check if the single argument is a positive integer
	num, err := strconv.Atoi(args[0])
	return err == nil && num > 0
}

// shouldRunCreate determines if the arguments indicate a create command
func shouldRunCreate(args []string) bool {
	if len(args) == 0 {
		return false
	}

	// Check if first arg is a known command or reserved word
	commands := []string{"ls", "view", "open", "peek", "delete", "path",
		"copy", "cp", "cleanup", "search", "nuke", "recover", "create", "new", "n",
		"version", "help", "completion", "pin", "unpin", "export", "show-data-file"}

	firstArg := strings.ToLower(args[0])
	for _, cmd := range commands {
		if firstArg == cmd {
			return false
		}
	}

	// If we have multiple args or a single quoted string with multiple words
	if len(args) > 1 {
		return true
	}

	// Check if single arg contains multiple words (was quoted)
	if len(args) == 1 && strings.Contains(args[0], " ") {
		return true
	}

	return false
}
