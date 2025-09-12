package cli

import (
	"os"
	"strconv"
	"time"

	"github.com/arthur-debert/padz/cmd/padz/store2"
	"github.com/arthur-debert/padz/internal/version"
	"github.com/arthur-debert/padz/pkg/commands"
	"github.com/arthur-debert/padz/pkg/config"
	"github.com/arthur-debert/padz/pkg/logging"
	"github.com/rs/zerolog/log"
	"github.com/spf13/cobra"
)

var (
	verbosity    int
	outputFormat string
	silent       bool
	verbose      bool
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
	rootCmd.PersistentFlags().CountVarP(&verbosity, "verbosity", "v", FlagVerboseDesc)
	rootCmd.PersistentFlags().Lookup("verbosity").Hidden = true
	rootCmd.PersistentFlags().StringVarP(&outputFormat, "format", "f", "term", FlagFormatDesc)

	// Output mode flags (mutually exclusive)
	rootCmd.PersistentFlags().BoolVar(&silent, "silent", false, "Suppress list output after commands")
	rootCmd.PersistentFlags().BoolVar(&verbose, "verbose", false, "Show list output after commands (default)")

	// Add version flag
	var versionFlag bool
	rootCmd.Flags().BoolVar(&versionFlag, "version", false, FlagVersionDesc)

	// Add hidden search flag to allow naked -s invocation
	var searchFlag string
	rootCmd.Flags().StringVarP(&searchFlag, "search", "s", "", "Search for scratches (redirects to ls -s)")
	rootCmd.Flags().Lookup("search").Hidden = true

	// Set PersistentPreRun for logging and output mode
	rootCmd.PersistentPreRun = func(cmd *cobra.Command, args []string) {
		// Setup logging based on verbosity
		logging.SetupLogger(verbosity)
		log.Debug().Str("command", cmd.Name()).Msg("Command started")

		// Handle output mode flags
		if silent && verbose {
			log.Fatal().Msg("Cannot use both --silent and --verbose flags")
		}

		// Default to verbose if neither flag is set
		if !silent && !verbose {
			verbose = true
		}

		// Run auto-cleanup for soft-deleted items (non-blocking, best effort)
		// Skip for help, completion, version commands, and commands that need immediate store access
		skipAutoCleanup := cmd.Name() == "help" || cmd.Name() == "completion" || cmd.Flags().Changed("version") ||
			cmd.Name() == "delete" || cmd.Name() == "restore" || cmd.Name() == "flush" || cmd.Name() == "nuke"

		if !skipAutoCleanup {
			go func() {
				// Small delay to let the main command complete first and avoid lock contention
				time.Sleep(100 * time.Millisecond)

				// Get current directory
				dir, err := os.Getwd()
				if err != nil {
					log.Debug().Err(err).Msg("Failed to get working directory for auto-cleanup")
					return
				}

				// Auto-cleanup soft-deleted items older than 7 days using StoreManager
				// This will cleanup in the current project store
				if _, err := commands.CleanupWithStoreManager(dir, 7); err != nil {
					log.Debug().Err(err).Msg("Auto-cleanup failed")
				}
			}()
		}
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

	// Use v2 store by default (Phase 5 migration complete)
	// Set PADZ_USE_V1=true to fall back to legacy store
	useV2 := os.Getenv("PADZ_USE_V1") != "true"

	// Single scratch commands
	if useV2 {
		// Use v2 implementations
		createCmd := store2.NewCreateCommand()
		createCmd.GroupID = "single"
		createCmd.Use = "create [content]"
		createCmd.Aliases = []string{"new", "n", "c"}
		createCmd.Short = "Create a new scratch (new, n, c)"
		rootCmd.AddCommand(createCmd)

		viewCmd := store2.NewViewCommand()
		viewCmd.GroupID = "single"
		viewCmd.Use = "view [id]"
		viewCmd.Aliases = []string{"show", "s", "v"}
		viewCmd.Short = "View a scratch (show, s, v)"
		rootCmd.AddCommand(viewCmd)
	} else {
		// Use v1 implementations
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
	}

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

	// Delete command will be added in the conditional section below

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
	if useV2 {
		// Use v2 list command
		lsCmd := store2.NewListCommand()
		lsCmd.GroupID = "multiple"
		lsCmd.Use = "list"
		lsCmd.Aliases = []string{"ls", "l"}
		lsCmd.Short = "List all scratches (ls, l)"
		rootCmd.AddCommand(lsCmd)

		// Use v2 search command
		searchCmd := store2.NewSearchCommand()
		searchCmd.GroupID = "multiple"
		searchCmd.Use = "search [term]"
		searchCmd.Short = "Search for scratches containing the given term"
		rootCmd.AddCommand(searchCmd)

		// Use v2 delete command
		deleteCmd := store2.NewDeleteCommand()
		deleteCmd.GroupID = "single"
		deleteCmd.Use = "delete [id]"
		deleteCmd.Aliases = []string{"del", "rm", "d"}
		deleteCmd.Short = "Delete a scratch (del, rm, d)"
		rootCmd.AddCommand(deleteCmd)
	} else {
		// Use v1 implementations
		lsCmd := newLsCmd()
		lsCmd.GroupID = "multiple"
		lsCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
		lsCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
		lsCmd.Flags().StringP("search", "s", "", "Search for scratches containing the given term")
		lsCmd.Flags().Bool("deleted", false, "Show only soft-deleted scratches")
		lsCmd.Flags().Bool("include-deleted", false, "Include soft-deleted scratches in listing")
		rootCmd.AddCommand(lsCmd)

		searchCmd := newSearchCmd()
		searchCmd.GroupID = "multiple"
		searchCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
		searchCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
		searchCmd.Flags().StringP("project", "p", "", "Limit search to specific project")
		rootCmd.AddCommand(searchCmd)

		deleteCmd := newDeleteCmd()
		deleteCmd.GroupID = "single"
		deleteCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
		deleteCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
		rootCmd.AddCommand(deleteCmd)
	}

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

	flushCmd := newFlushCmd()
	flushCmd.GroupID = "multiple"
	rootCmd.AddCommand(flushCmd)

	restoreCmd := newRestoreCmd()
	restoreCmd.GroupID = "multiple"
	rootCmd.AddCommand(restoreCmd)

	exportCmd := newExportCmd()
	exportCmd.GroupID = "multiple"
	exportCmd.Flags().BoolP("all", "a", false, FlagAllDesc)
	exportCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(exportCmd)

	// Utility commands (not grouped)
	showDataFileCmd := newShowDataFileCmd()
	showDataFileCmd.Flags().BoolP("global", "g", false, FlagGlobalDesc)
	rootCmd.AddCommand(showDataFileCmd)

	// Legacy store2 subcommand (for compatibility)
	// Remove this in future versions once migration is complete
	if !useV2 {
		store2Cmd := store2.NewStore2Command()
		rootCmd.AddCommand(store2Cmd)
	}

	return rootCmd
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main(). It only needs to happen once to the rootCmd.
func Execute() error {
	args := os.Args[1:]

	// Resolve the command to run based on arguments
	resolvedArgs := resolveCommand(args)

	// Create root command and execute with resolved args
	rootCmd := NewRootCmd()
	rootCmd.SetArgs(resolvedArgs)
	return rootCmd.Execute()
}

// resolveCommand determines the appropriate command based on the given arguments.
// It returns the potentially modified argument list with an explicit command inserted.
func resolveCommand(args []string) []string {
	// Case 1: No arguments -> run list command
	if len(args) == 0 {
		return []string{"list"}
	}

	// Case 2: Single integer argument -> run view/open command
	if len(args) == 1 {
		if num, err := strconv.Atoi(args[0]); err == nil && num > 0 {
			cmd := config.NakedIntCommand // "view" or "open"
			return append([]string{cmd}, args...)
		}
	}

	// Case 3: Check if first arg is a flag (starts with -)
	if len(args) > 0 && len(args[0]) > 0 && args[0][0] == '-' {
		// Special handling for help and version flags
		if args[0] == "--help" || args[0] == "-h" || args[0] == "--version" {
			return args // Let Cobra handle these
		}
		// Check if it's a create flag (-t for title)
		if args[0] == "-t" || args[0] == "--title" {
			return append([]string{"create"}, args...)
		}
		// Other flags imply list command
		return append([]string{"list"}, args...)
	}

	// Case 4: Check if first arg is a known command (including help)
	if len(args) > 0 {
		rootCmd := NewRootCmd()

		// Check for built-in commands that Cobra handles
		if args[0] == "help" || args[0] == "completion" {
			return args
		}

		// Try to find the command
		cmd, _, err := rootCmd.Find(args)
		if err == nil && cmd != nil && cmd.Name() != rootCmd.Name() {
			// Found a valid subcommand (not root)
			return args
		}
	}

	// Case 5: No valid command found, assume it's a create command
	return append([]string{"create"}, args...)
}
