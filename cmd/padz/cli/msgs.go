package cli

// Root command messages
const (
	RootUse   = "padz"
	RootShort = "padz create scratch pads, draft files using $EDITOR."
	RootLong  = `padz create scratch pads, draft files using $EDITOR.

  $ padz                  # Lists scratches with an index to be used in open, view, delete:
      1. 10 minutes ago My first scratch note
  $ padz create             # create a new scratch in $EDITOR
  $ padz "My scratch title. Can have content"  # shortcut to create
  $ padz view <index>       # views in shell
  $ padz ls -s "<term>"     # search for scratches containing term`

	// Root command error messages
	ErrFailedToInitStore     = "Failed to initialize store"
	ErrFailedToGetWorkingDir = "Failed to get working directory"
	ErrFailedToGetProject    = "Failed to get current project"
	ErrFailedToCreateNote    = "Failed to create note"

	// Version format
	VersionFormat = "padz version %s (commit: %s, built: %s)\n"

	// Command groups
	GroupSingleScratch = "SINGLE SCRATCH:"
	GroupScratches     = "SCRATCHES:"
)

// Flag messages
const (
	// Global flags
	FlagVerboseDesc = "Increase verbosity (-v, -vv, -vvv)"
	FlagFormatDesc  = "Output format (plain, json, term)"
	FlagVersionDesc = "Print version information"

	// Common flags used across commands
	FlagGlobalDesc       = "Show only global scratches"
	FlagGlobalDescSearch = "Search in global scratches only"
)

// LS command messages
const (
	LsUse   = "ls"
	LsShort = "Lists all scratches for the current project"
	LsLong  = `Lists all scratches for the current project.
The output includes the index, the relative time of creation, and the title of the scratch.`

	LsNoScratchesFound = "No scratches found."
)

// View command messages
const (
	ViewUse   = "view <index>"
	ViewShort = "View a scratch (v)"
	ViewLong  = "View the content of a scratch identified by its index."
)

// Open command messages
const (
	OpenUse   = "open <index>"
	OpenShort = "Open a scratch in $EDITOR (o, e)"
	OpenLong  = "Open a scratch, identified by its index, in $EDITOR."

	OpenSuccess = "Scratch updated."
)

// Peek command messages
const (
	PeekUse   = "peek <index>"
	PeekShort = "Peek at a scratch"
	PeekLong  = "Peek at the first and last lines of a scratch."

	FlagLinesDesc = "Number of lines to show from the beginning and end"
)

// Delete command messages
const (
	DeleteUse   = "delete <index>"
	DeleteShort = "Delete a scratch (rm, d, del)"
	DeleteLong  = "Delete a scratch identified by its index."

	DeleteSuccess = "Scratch deleted."
)

// Cleanup command messages
const (
	CleanupUse   = "cleanup"
	CleanupShort = "Cleanup old scratches (clean)"
	CleanupLong  = "Cleanup scratches older than a specified number of days."

	FlagDaysDesc = "Delete scratches older than this many days"

	CleanupSuccessFormat = "Cleaned up scratches older than %d days."
)

// Copy command messages
const (
	CopyUse   = "copy <index>"
	CopyShort = "Copy a scratch to the clipboard (cp)"
	CopyLong  = `Copy the content of a scratch to the system clipboard.

The scratch is identified by its index number from the 'padz ls' output.
Use the --all flag to select from all scratches across all projects.`

	ErrFailedToCopyScratch   = "Failed to copy scratch to clipboard"
	SuccessCopiedToClipboard = "Copied to clipboard"
)

// Search command messages
const (
	SearchUse   = "search [term]"
	SearchShort = "Search for a scratch"
	SearchLong  = "Search for a scratch by a regular expression."

	SearchNoMatchesFound = "No matches found."
)

// Nuke command messages
const (
	NukeUse   = "nuke"
	NukeShort = "Delete all scratches in the current scope"
	NukeLong  = `Delete all scratches in the current scope (project or global).
Use --all to delete all scratches across all scopes.`

	// Confirmation prompts
	NukeConfirmProject = "This will delete all %d pads in [%s]. Confirm? [y/N] "
	NukeConfirmGlobal  = "This will delete all %d pads in global storage. Confirm? [y/N] "
	NukeConfirmAll     = "This will delete all %d pads across all scopes, projects and global. Confirm? [y/N] "

	// Success messages
	NukeSuccessProject = "Deleted all %d pads in [%s]."
	NukeSuccessGlobal  = "Deleted all %d pads in global storage."
	NukeSuccessAll     = "Deleted all %d pads across all scopes."

	// Other messages
	NukeNoPadsFound = "No pads found to delete."
	NukeCancelled   = "Nuke cancelled."
)

// ShowDataFile command messages
const (
	ShowDataFileUse   = "show-data-file"
	ShowDataFileShort = "Show the path to the data directory used by padz"
	ShowDataFileLong  = `Show the path to the data directory used by padz.

This command displays the directory where padz stores all scratch files and metadata.
Note that both global and local scratches are stored in the same location - the --global
flag only affects which scratches are filtered/displayed, not where they are stored.`
)
