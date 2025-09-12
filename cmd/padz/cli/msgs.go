package cli

// Root command messages
const (
	RootUse   = "padz"
	RootShort = "padz create scratch pads, draft files using $EDITOR."
	RootLong  = `padz create scratch pads, draft files using $EDITOR.

  $ padz                    # Lists pads with an index to be used in open, view, delete:
      1. 10 minutes ago My first pad note
  $ padz create             # create a new pad in $EDITOR
  $ padz "My pad title. Can have content"  # shortcut to create
  $ padz view <index>       # views in shell
  $ padz list -s "<term>"   # search for pads containing term`

	// Version format
	VersionFormat = "padz version %s (commit: %s, built: %s)\n"

	// Command groups
	GroupSinglePad = "SINGLE PAD:"
	GroupPads      = "PADS:"
)

// Flag messages
const (
	// Global flags
	FlagVerboseDesc = "Increase verbosity (-v, -vv, -vvv)"
	FlagFormatDesc  = "Output format (plain, json, term)"
	FlagVersionDesc = "Print version information"

	// Common flags used across commands
	FlagAllDesc          = "Show pads from all scopes"
	FlagAllDescSearch    = "Search in all scopes"
	FlagGlobalDesc       = "Show only global pads"
	FlagGlobalDescSearch = "Search in global pads only"
)

// List command messages
const (
	ListUse   = "list"
	ListAlias = "ls"
	ListShort = "Lists all pads for the current scope"
	ListLong  = `Lists all pads for the current scope.
The output includes the index, the relative time of creation, and the title of the pad.`

	ListNoPadsFound = "No pads found."
)

// View command messages
const (
	ViewUse     = "view <index>"
	ViewAliases = "v"
	ViewShort   = "View a pad (v)"
	ViewLong    = "View the content of a pad identified by its index."
)

// Open command messages
const (
	OpenUse     = "open <index>"
	OpenAliases = "o,e"
	OpenShort   = "Open a pad in $EDITOR (o, e)"
	OpenLong    = "Open a pad, identified by its index, in $EDITOR."

	OpenSuccess = "Pad updated."
)

// Peek command messages
const (
	PeekUse   = "peek <index>"
	PeekShort = "Peek at a pad"
	PeekLong  = "Peek at the first and last lines of a pad."

	FlagLinesDesc = "Number of lines to show from the beginning and end"
)

// Delete command messages
const (
	DeleteUse     = "delete <index>"
	DeleteAliases = "rm,d,del"
	DeleteShort   = "Delete a pad (rm, d, del)"
	DeleteLong    = "Delete a pad identified by its index."

	DeleteSuccess = "Pad deleted."
)

// Create command messages
const (
	CreateUse   = "create [content]"
	CreateShort = "Create a new pad"
	CreateLong  = `Create a new pad in $EDITOR or with provided content.

If content is provided as arguments, it will be used as the initial content.
Otherwise, $EDITOR will be opened for content creation.`

	CreateSuccess = "Pad created."
)

// Cleanup command messages
const (
	CleanupUse     = "cleanup"
	CleanupAliases = "clean"
	CleanupShort   = "Cleanup old pads (clean)"
	CleanupLong    = "Cleanup pads older than a specified number of days."

	FlagDaysDesc = "Delete pads older than this many days"

	CleanupSuccessFormat = "Cleaned up pads older than %d days."
)

// Copy command messages
const (
	CopyUse     = "copy <index>"
	CopyAliases = "cp"
	CopyShort   = "Copy a pad to the clipboard (cp)"
	CopyLong    = `Copy the content of a pad to the system clipboard.

The pad is identified by its index number from the 'padz list' output.
Use the --all flag to select from all pads across all scopes.`

	ErrFailedToCopyPad       = "Failed to copy pad to clipboard"
	SuccessCopiedToClipboard = "Copied to clipboard"
)

// Search command messages
const (
	SearchUse   = "search [term]"
	SearchShort = "Search for a pad"
	SearchLong  = "Search for a pad by a regular expression."

	SearchNoMatchesFound = "No matches found."
)

// Pin/Unpin command messages
const (
	PinUse   = "pin <index>"
	PinShort = "Pin pads for quick access"
	PinLong  = "Pin one or more pads for quick access."

	UnpinUse   = "unpin <index>"
	UnpinShort = "Unpin pads"
	UnpinLong  = "Unpin one or more pads."

	PinSuccess   = "Pad pinned."
	UnpinSuccess = "Pad unpinned."
)

// Export command messages
const (
	ExportUse   = "export <index>"
	ExportShort = "Export pads to files"
	ExportLong  = `Export one or more pads to files.

Supported formats: txt (default), md (markdown)
Files will be exported to the current directory unless specified otherwise.`

	ExportSuccess = "Pad exported."
)

// Flush/Restore command messages
const (
	FlushUse   = "flush [index]"
	FlushShort = "Permanently delete soft-deleted pads"
	FlushLong  = "Permanently delete soft-deleted pads. Use --all to flush all deleted pads."

	RestoreUse   = "restore <index>"
	RestoreShort = "Restore soft-deleted pads"
	RestoreLong  = "Restore one or more soft-deleted pads."

	FlushSuccess   = "Pad permanently deleted."
	RestoreSuccess = "Pad restored."
)

// Nuke command messages
const (
	NukeUse   = "nuke"
	NukeShort = "Delete all pads in the current scope"
	NukeLong  = `Delete all pads in the current scope (project or global).
Use --all to delete all pads across all scopes.`

	// Confirmation prompts
	NukeConfirmScope = "This will delete all %d pads in scope '%s'. Confirm? [y/N] "
	NukeConfirmAll   = "This will delete all %d pads across all scopes. Confirm? [y/N] "

	// Success messages
	NukeSuccessScope = "Deleted all %d pads in scope '%s'."
	NukeSuccessAll   = "Deleted all %d pads across all scopes."

	// Other messages
	NukeNoPadsFound = "No pads found to delete."
	NukeCancelled   = "Nuke cancelled."
)

// ShowDataFile command messages
const (
	ShowDataFileUse   = "show-data-file"
	ShowDataFileShort = "Show the path to the data directory used by padz"
	ShowDataFileLong  = `Show the path to the data directory used by padz.

This command displays the directory where padz stores all pad files and metadata.
Note that both global and local pads are stored in the same location - the --global
flag only affects which pads are filtered/displayed, not where they are stored.`
)

// Path command messages
const (
	PathUse   = "path [index]"
	PathShort = "Show file system paths for pads or stores"
	PathLong  = `Show the file system paths for pads or stores.

If no index is provided, shows the current scope's store path.
If an index is provided, shows the path to that specific pad's content file.`
)
