package cli

// Root command messages
const (
	RootUse   = "padz"
	RootShort = "padz create scratch pads, draft files using $EDITOR."
	RootLong  = `padz create scratch pads, draft files using $EDITOR.

  $ padz                    # edit a new scratch in $EDITOR
  $ padz ls                 # Lists scratches with an index to be used in open, view, delete:
      1. 10 minutes ago My first scratch note
  $ padz view <index>       # views in shell
  $ padz search "<term>"    # search for scratches containing term`

	// Root command error messages
	ErrFailedToInitStore      = "Failed to initialize store"
	ErrFailedToGetWorkingDir  = "Failed to get working directory"
	ErrFailedToGetProject     = "Failed to get current project"
	ErrFailedToCreateNote     = "Failed to create note"

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
	FlagAllDesc       = "Show scratches from all projects"
	FlagAllDescSearch = "Search in all projects"
	FlagGlobalDesc    = "Show only global scratches"
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
	ViewShort = "View a scratch"
	ViewLong  = "View the content of a scratch identified by its index."
)

// Open command messages
const (
	OpenUse   = "open <index>"
	OpenShort = "Open a scratch in $EDITOR"
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
	DeleteShort = "Delete a scratch"
	DeleteLong  = "Delete a scratch identified by its index."
	
	DeleteSuccess = "Scratch deleted."
)

// Cleanup command messages
const (
	CleanupUse   = "cleanup"
	CleanupShort = "Cleanup old scratches"
	CleanupLong  = "Cleanup scratches older than a specified number of days."
	
	FlagDaysDesc = "Delete scratches older than this many days"
	
	CleanupSuccessFormat = "Cleaned up scratches older than %d days."
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