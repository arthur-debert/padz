package cli

import (
	"os"
	"strconv"
	"time"

	"github.com/arthur-debert/padz/pkg/store"
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
	// Create a fresh root command each time
	rootCmd := &cobra.Command{
		Use:               RootUse,
		Short:             RootShort,
		Long:              RootLong,
		DisableAutoGenTag: true,
		CompletionOptions: cobra.CompletionOptions{
			DisableDefaultCmd: true,
		},
		PersistentPreRun: func(cmd *cobra.Command, args []string) {
			// Handle output mode flags
			if silent && verbose {
				// Cannot use both --silent and --verbose flags
				os.Exit(1)
			}

			// Default to verbose if neither flag is set
			if !silent && !verbose {
				verbose = true
			}

			// Run auto-cleanup for soft-deleted items (non-blocking, best effort)
			// Skip for help, completion, version commands, and commands that need immediate store access
			skipAutoCleanup := cmd.Name() == "help" || cmd.Name() == "completion" ||
				cmd.Name() == "delete" || cmd.Name() == "restore" || cmd.Name() == "flush" || cmd.Name() == "nuke"

			if !skipAutoCleanup {
				go func() {
					// Small delay to let the main command complete first and avoid lock contention
					time.Sleep(100 * time.Millisecond)

					// Auto-cleanup soft-deleted items older than 7 days
					autoCleanupDeletedPads(7)
				}()
			}
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
	rootCmd.Flags().StringVarP(&searchFlag, "search", "s", "", "Search for pads (redirects to list -s)")
	rootCmd.Flags().Lookup("search").Hidden = true

	// Set up command groups
	rootCmd.AddGroup(&cobra.Group{
		ID:    "single",
		Title: GroupSinglePad,
	})
	rootCmd.AddGroup(&cobra.Group{
		ID:    "multiple",
		Title: GroupPads,
	})

	// Single pad commands
	createCmd := newCreateCommand()
	createCmd.GroupID = "single"
	rootCmd.AddCommand(createCmd)

	viewCmd := newViewCommand()
	viewCmd.Aliases = []string{"v"}
	viewCmd.GroupID = "single"
	rootCmd.AddCommand(viewCmd)

	openCmd := newOpenCommand()
	openCmd.Aliases = []string{"o", "e"}
	openCmd.GroupID = "single"
	rootCmd.AddCommand(openCmd)

	peekCmd := newPeekCommand()
	peekCmd.GroupID = "single"
	rootCmd.AddCommand(peekCmd)

	deleteCmd := newDeleteCommand()
	deleteCmd.Aliases = []string{"rm", "d", "del"}
	deleteCmd.GroupID = "single"
	rootCmd.AddCommand(deleteCmd)

	pathCmd := newPathCommand()
	pathCmd.GroupID = "single"
	rootCmd.AddCommand(pathCmd)

	copyCmd := newCopyCommand()
	copyCmd.Aliases = []string{"cp"}
	copyCmd.GroupID = "single"
	rootCmd.AddCommand(copyCmd)

	pinCmd := newPinCommand()
	pinCmd.GroupID = "single"
	rootCmd.AddCommand(pinCmd)

	unpinCmd := newUnpinCommand()
	unpinCmd.GroupID = "single"
	rootCmd.AddCommand(unpinCmd)

	// Multiple pads commands
	listCmd := newListCommand()
	listCmd.Aliases = []string{"ls"}
	listCmd.GroupID = "multiple"
	rootCmd.AddCommand(listCmd)

	searchCmd := newSearchCommand()
	searchCmd.GroupID = "multiple"
	rootCmd.AddCommand(searchCmd)

	cleanupCmd := newCleanupCommand()
	cleanupCmd.Aliases = []string{"clean"}
	cleanupCmd.GroupID = "multiple"
	rootCmd.AddCommand(cleanupCmd)

	nukeCmd := newNukeCommand()
	nukeCmd.GroupID = "multiple"
	rootCmd.AddCommand(nukeCmd)

	flushCmd := newFlushCommand()
	flushCmd.GroupID = "multiple"
	rootCmd.AddCommand(flushCmd)

	restoreCmd := newRestoreCommand()
	restoreCmd.GroupID = "multiple"
	rootCmd.AddCommand(restoreCmd)

	exportCmd := newExportCommand()
	exportCmd.GroupID = "multiple"
	rootCmd.AddCommand(exportCmd)

	recoverCmd := newRecoverCommand()
	recoverCmd.GroupID = "multiple"
	rootCmd.AddCommand(recoverCmd)

	// Utility commands (not grouped)
	showDataFileCmd := newShowDataFileCommand()
	rootCmd.AddCommand(showDataFileCmd)

	return rootCmd
}

// Execute adds all child commands to the root command and sets flags appropriately.
// This is called by main.main().
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

	// Case 2: Single integer argument -> run view command
	if len(args) == 1 {
		if num, err := strconv.Atoi(args[0]); err == nil && num > 0 {
			return append([]string{"view"}, args...)
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
		// Check if it's a search flag (-s)
		if args[0] == "-s" || args[0] == "--search" {
			return append([]string{"list"}, args...)
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

// autoCleanupDeletedPads automatically removes soft-deleted pads older than the specified days
func autoCleanupDeletedPads(daysOld int) {
	// Get base store directory
	baseDir, err := store.GetStorePath("")
	if err != nil {
		return // Silently fail
	}

	// Find all scope directories
	entries, err := os.ReadDir(baseDir)
	if err != nil {
		return // Silently fail
	}

	cutoffTime := time.Now().AddDate(0, 0, -daysOld)

	for _, entry := range entries {
		if !entry.IsDir() {
			continue
		}

		scope := entry.Name()
		storePath, err := store.GetStorePath(scope)
		if err != nil {
			continue
		}

		storeInstance, err := store.NewStore(storePath)
		if err != nil {
			continue
		}

		// Get deleted pads
		deletedPads, err := storeInstance.ListDeleted()
		if err != nil {
			continue
		}

		// Flush pads older than cutoff
		for _, pad := range deletedPads {
			if pad.DeletedAt != nil && pad.DeletedAt.Before(cutoffTime) {
				_, _ = storeInstance.Flush(pad.UserID) // Ignore errors
			}
		}
	}
}

// IsVerboseMode returns true if verbose output is enabled
func IsVerboseMode() bool {
	return verbose && !silent
}

// IsSilentMode returns true if silent output is enabled
func IsSilentMode() bool {
	return silent
}

// GetOutputFormat returns the current output format
func GetOutputFormat() string {
	return outputFormat
}
