use super::complete::{
    active_pads_completer, all_pads_completer, archived_pads_completer, deleted_pads_completer,
};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use once_cell::sync::Lazy;
use standout::cli::{
    render_help_with_topics, App, CommandGroup, DefaultCommandContext, Dispatch, HelpConfig,
};
use standout::topics::TopicRegistry;
use standout::OutputMode;

// Import handlers module
use super::handlers;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

impl CompletionShell {
    /// The shell name as expected by clap_complete's COMPLETE env var.
    pub fn as_complete_env(self) -> &'static str {
        match self {
            CompletionShell::Bash => "bash",
            CompletionShell::Zsh => "zsh",
            CompletionShell::Fish => "fish",
        }
    }
}

/// Returns the version string, including git hash and commit date for non-release builds.
/// Format for releases: "v0.8.10"
/// Format for dev builds: "v0.8.10\ndev: abc1234 2024-01-15 14:30"
fn get_version() -> &'static str {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    const GIT_HASH: &str = env!("GIT_HASH");
    const GIT_COMMIT_DATE: &str = env!("GIT_COMMIT_DATE");
    const IS_RELEASE: &str = env!("IS_RELEASE");

    // Use a static to compute the version string once
    use std::sync::OnceLock;
    static VERSION_STRING: OnceLock<String> = OnceLock::new();

    VERSION_STRING.get_or_init(|| {
        if IS_RELEASE == "true" {
            format!("v{}", VERSION)
        } else {
            format!("v{}\ndev: {} {}", VERSION, GIT_HASH, GIT_COMMIT_DATE)
        }
    })
}

#[derive(Parser, Debug)]
#[command(
    name = "padz",
    bin_name = "padz",
    version = get_version(),
    disable_help_subcommand = true,
    after_help = "Enable shell completions:\n  padz completion install"
)]
#[command(about = "Context-aware command-line note-taking tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Operate on global pads
    #[arg(short, long, global = true, conflicts_with = "data")]
    pub global: bool,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// Override data directory path (e.g., for git worktrees)
    #[arg(long, global = true, value_name = "PATH", conflicts_with = "global")]
    pub data: Option<String>,
}

// Help topics registry - loaded from topics directory
static HELP_TOPICS: Lazy<TopicRegistry> = Lazy::new(|| {
    let mut registry = TopicRegistry::new();
    // Topics are embedded at compile time from the topics directory
    // We manually add them since include_str! requires compile-time paths
    let topic_content = include_str!("topics/scopes.txt");
    if let Some(topic) = parse_topic_file("scopes", topic_content) {
        registry.add_topic(topic);
    }
    registry
});

/// Builds an `App` carrying padz's help topics.
///
/// Two Standout 7.6 changes are absorbed here:
///
/// - The topic registry is private behind `registry_mut()` (6.2's
///   `App::with_registry` constructor is gone), so it is installed after
///   construction rather than passed in.
/// - `help_handling` now defaults to off and must be opted into. It was
///   implicit in 6.2, and it is what installs the `help` subcommand and routes
///   `--help`/topics through Standout. 7.6 additionally rejects an app that
///   configures topics or command groups without it.
fn app_with_topics() -> App {
    let mut app = App::new()
        .help_handling(true)
        .default_command_with(invocation_default_command);
    *app.registry_mut() = HELP_TOPICS.clone();
    app
}

/// Select the command for a successfully parsed naked invocation without
/// consuming stdin. Explicit commands, help, version, and usage errors never
/// reach this resolver; Standout centralizes those precedence rules.
pub(super) fn invocation_default_command(ctx: &DefaultCommandContext<'_>) -> Option<String> {
    Some(
        if ctx.stdin_is_piped() {
            "create"
        } else {
            "list"
        }
        .to_string(),
    )
}

/// Parse a topic file content into a Topic struct
fn parse_topic_file(name: &str, content: &str) -> Option<standout::topics::Topic> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 2 {
        return None;
    }

    // First non-blank line is title
    let title_idx = lines.iter().position(|l| !l.trim().is_empty())?;
    let title = lines[title_idx].trim().to_string();

    // Rest is content (skip blank lines after title)
    let content_lines = &lines[title_idx + 1..];
    let content_start = content_lines
        .iter()
        .position(|l| !l.trim().is_empty())
        .unwrap_or(content_lines.len());

    let body = content_lines[content_start..].join("\n");
    if body.trim().is_empty() {
        return None;
    }

    Some(standout::topics::Topic::new(
        title,
        body,
        standout::topics::TopicType::Text,
        Some(name.to_string()),
    ))
}

/// Builds the clap Command for use with CompleteEnv.
/// This is called by the completion system before normal parsing.
pub fn build_command() -> clap::Command {
    Cli::command()
}

/// Parses command-line arguments using standout's App.
/// This handles help display (including topics) and errors automatically.
/// It also adds the `--output` flag, which standout defines over the full mode set:
/// `auto`, `term`, `text`, `term-debug`, `json`, `yaml`, `xml`, and `csv`.
/// Returns the parsed CLI and the output mode extracted from the matches.
///
/// The mode is extracted by standout's own [`App::extract_output_mode`] rather than by
/// a local match. Standout owns the `--output` value set: a hand-written parser here is
/// a copy that silently rots when standout adds a mode. It did — the local copy knew
/// only `json`, so `--output yaml|xml|csv` parsed as valid clap values and then fell
/// through to `Auto`, quietly rendering the human template (ANSI, glyphs, width
/// truncation) to callers who had asked for machine-readable data. Delegating keeps the
/// mode set defined in exactly one place.
pub fn parse_cli() -> (Cli, OutputMode) {
    // Intercept top-level help to show grouped output
    if should_show_custom_help() {
        println!("{}", render_custom_help());
        std::process::exit(0);
    }

    let app: App = app_with_topics();
    let matches = app.parse_with(Cli::command());
    let output_mode = app.extract_output_mode(&matches);

    let cli = Cli::from_arg_matches(&matches).expect("Failed to parse CLI arguments");
    (cli, output_mode)
}

/// Returns the help output as a styled string (used for empty list display).
pub fn get_grouped_help() -> String {
    render_custom_help()
}

/// Checks if the current invocation is a top-level help request
/// (not subcommand help like `padz create --help` or `padz help create`).
fn should_show_custom_help() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let subcommands = [
        "create",
        "n",
        "list",
        "ls",
        "search",
        "peek",
        "pk",
        "view",
        "v",
        "edit",
        "e",
        "open",
        "o",
        "delete",
        "rm",
        "move",
        "mv",
        "restore",
        "archive",
        "unarchive",
        "pin",
        "p",
        "unpin",
        "u",
        "path",
        "uuid",
        "complete",
        "done",
        "reopen",
        "purge",
        "export",
        "import",
        "clone",
        "migrate",
        "tag",
        "doctor",
        "config",
        "init",
        "completion",
    ];

    // `padz help` with no further args
    if args.len() == 1 && args[0] == "help" {
        return true;
    }

    // --help/-h before any subcommand means top-level help
    for arg in &args {
        if arg == "--help" || arg == "-h" {
            return true;
        }
        if subcommands.contains(&arg.as_str()) {
            return false;
        }
    }

    false
}

/// Returns the command groups for organized help display.
fn command_groups() -> Vec<CommandGroup> {
    vec![
        CommandGroup {
            title: "Commands".into(),
            help: None,
            commands: vec![
                Some("init".into()),
                Some("create".into()),
                Some("list".into()),
                Some("search".into()),
            ],
        },
        CommandGroup {
            title: "Per Pad(s)".into(),
            help: Some("These commands accept one or more pad ids (<id>...)".into()),
            commands: vec![
                Some("open".into()),
                Some("view".into()),
                Some("copy".into()),
                Some("peek".into()),
                Some("move".into()),
                Some("delete".into()),
                None,
                Some("archive".into()),
                Some("unarchive".into()),
                None,
                Some("pin".into()),
                Some("unpin".into()),
                Some("path".into()),
                Some("uuid".into()),
                None,
                Some("complete".into()),
                Some("reopen".into()),
                None,
                Some("import".into()),
                Some("export".into()),
                Some("clone".into()),
                Some("migrate".into()),
                None,
                Some("tag".into()),
            ],
        },
        CommandGroup {
            title: "Misc".into(),
            help: None,
            commands: vec![
                Some("purge".into()),
                Some("restore".into()),
                None,
                Some("completion".into()),
                Some("help".into()),
                Some("doctor".into()),
                Some("config".into()),
            ],
        },
    ]
}

/// Renders the custom grouped help output with standout styling.
fn render_custom_help() -> String {
    let app = app_with_topics();
    let cmd = app.augment_command_with_help(Cli::command());

    let config = HelpConfig {
        command_groups: Some(command_groups()),
        ..Default::default()
    };

    let mut help = render_help_with_topics(&cmd, &HELP_TOPICS, Some(config))
        .unwrap_or_else(|e| format!("Help rendering error: {}", e));

    help.push_str("\n\nEnable shell completions:\n");
    help.push_str("  padz completion install");

    help
}

/// All padz commands in a flat enum with Dispatch derive for automatic handler routing
#[derive(Subcommand, Dispatch, Debug)]
#[dispatch(handlers = handlers)]
pub enum Commands {
    // --- Core commands ---
    /// Create a new pad
    ///
    /// The content source (args / piped stdin / editor) is resolved before
    /// dispatch by `cli::input`'s chain; the handler receives the decision.
    #[command(alias = "n", display_order = 1)]
    #[dispatch(skip)]
    Create {
        /// Force opening the editor (even in todos mode)
        #[arg(long, short = 'e', conflicts_with = "no_editor")]
        editor: bool,

        /// Skip opening the editor
        #[arg(long)]
        no_editor: bool,

        /// Create inside another pad (parent selector, e.g. 1 or p1)
        #[arg(long, short = 'i')]
        inside: Option<String>,

        /// File format for this pad (e.g., md, txt, markdown, text). Overrides global setting.
        #[arg(long, short = 'f')]
        format: Option<String>,

        /// Title words (joined with spaces, optional - opens empty editor if not provided)
        #[arg(trailing_var_arg = true)]
        title: Vec<String>,
    },

    /// List pads
    #[command(alias = "ls", display_order = 2)]
    #[dispatch(pure)]
    List {
        /// Pad IDs to show (e.g. 2, 3 5, 1-3)
        #[arg(num_args = 0..)]
        ids: Vec<String>,

        /// Search term
        #[arg(short, long)]
        search: Option<String>,

        /// Show deleted pads
        #[arg(long, conflicts_with = "all")]
        deleted: bool,

        /// Show archived pads
        #[arg(long, conflicts_with = "all")]
        archived: bool,

        /// Show all pads (active, archived, and deleted)
        #[arg(long)]
        all: bool,

        /// Peek at pad content
        #[arg(long)]
        peek: bool,

        /// Show only planned pads
        #[arg(long, conflicts_with_all = ["completed", "in_progress"])]
        planned: bool,

        /// Show only completed pads
        #[arg(long, conflicts_with_all = ["planned", "in_progress"])]
        completed: bool,

        /// Show only in-progress pads
        #[arg(long, conflicts_with_all = ["planned", "completed"])]
        in_progress: bool,

        /// Filter by tag(s) (can be specified multiple times, uses AND logic)
        #[arg(long = "tag", short = 't', num_args = 1..)]
        tags: Vec<String>,

        /// Show short UUIDs next to pad titles
        #[arg(long)]
        uuid: bool,

        /// Show status icons (even in notes mode)
        #[arg(long)]
        show_status: bool,
    },

    /// Search pads (dedicated command)
    #[command(display_order = 3)]
    #[dispatch(pure, template = "list")]
    Search {
        /// Search term
        term: String,

        /// Show deleted pads
        #[arg(long, conflicts_with = "all")]
        deleted: bool,

        /// Show archived pads
        #[arg(long, conflicts_with = "all")]
        archived: bool,

        /// Show all pads (active, archived, and deleted)
        #[arg(long)]
        all: bool,

        /// Show only completed pads
        #[arg(long)]
        completed: bool,

        /// Filter by tag(s) (can be specified multiple times, uses AND logic)
        #[arg(long = "tag", short = 't', num_args = 1..)]
        tags: Vec<String>,

        /// Show short UUIDs next to pad titles
        #[arg(long)]
        uuid: bool,
    },

    /// Peek at pad content previews
    #[command(alias = "pk", display_order = 4)]
    #[dispatch(pure, template = "list")]
    Peek {
        /// Pad IDs to show (e.g. 2, 3 5, 1-3)
        #[arg(num_args = 0..)]
        ids: Vec<String>,

        /// Filter by tag(s) (can be specified multiple times, uses AND logic)
        #[arg(long = "tag", short = 't', num_args = 1..)]
        tags: Vec<String>,

        /// Show short UUIDs next to pad titles
        #[arg(long)]
        uuid: bool,
    },

    // --- Pad operations ---
    /// View one or more pads
    #[command(alias = "v", display_order = 10)]
    #[dispatch(pure, template = "view")]
    View {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = all_pads_completer())]
        indexes: Vec<String>,

        /// Peek at pad content
        #[arg(long)]
        peek: bool,

        /// Show UUID in view output
        #[arg(long)]
        uuid: bool,

        /// Show only the selected pad(s), no children
        #[arg(long, conflicts_with_all = ["tree", "indented"])]
        flat: bool,

        /// Recursively include children (default)
        #[arg(long, conflicts_with_all = ["flat", "indented"])]
        tree: bool,

        /// Recursively include children with 4-space indentation per level
        #[arg(long, conflicts_with_all = ["flat", "tree"])]
        indented: bool,
    },

    /// Copy one or more pads to the clipboard (without printing)
    #[command(alias = "cp", display_order = 10)]
    #[dispatch(pure, template = "copy")]
    Copy {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = all_pads_completer())]
        indexes: Vec<String>,

        /// Peek at pad content
        #[arg(long)]
        peek: bool,

        /// Show only the selected pad(s), no children
        #[arg(long, conflicts_with_all = ["tree", "indented"])]
        flat: bool,

        /// Recursively include children (default)
        #[arg(long, conflicts_with_all = ["flat", "indented"])]
        tree: bool,

        /// Recursively include children with 4-space indentation per level
        #[arg(long, conflicts_with_all = ["flat", "tree"])]
        indented: bool,
    },

    /// Edit a pad in the editor
    #[command(alias = "e", display_order = 11, hide = true)]
    #[dispatch(skip)]
    Edit {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Open a pad in the editor (alias for edit)
    ///
    /// Shares `edit`'s handler, and therefore needs `edit`'s input chain: the
    /// handler reads its content decision from the same named input.
    #[command(alias = "o", display_order = 12)]
    #[dispatch(skip)]
    Open {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = all_pads_completer())]
        indexes: Vec<String>,
    },

    /// Delete one or more pads (protected pads must be unpinned first)
    #[command(alias = "rm", display_order = 13)]
    #[dispatch(pure, template = "modification_result")]
    Delete {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(num_args = 1.., add = active_pads_completer(), required_unless_present = "completed")]
        indexes: Vec<String>,

        /// Delete all pads marked as completed
        #[arg(long = "completed", conflicts_with = "indexes")]
        completed: bool,
    },

    /// Restore deleted pads
    #[command(display_order = 14)]
    #[dispatch(pure, template = "modification_result")]
    Restore {
        /// Indexes of deleted pads (e.g. d1 d2 or just 1 2)
        #[arg(required = true, num_args = 1.., add = deleted_pads_completer())]
        indexes: Vec<String>,
    },

    /// Archive pads (move to cold storage)
    #[command(display_order = 15)]
    #[dispatch(pure, template = "modification_result")]
    Archive {
        /// Indexes of pads to archive (e.g. 1 3 5)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Unarchive pads (restore from archive)
    #[command(display_order = 16)]
    #[dispatch(pure, template = "modification_result")]
    Unarchive {
        /// Indexes of archived pads (e.g. ar1 ar2 or just 1 2)
        #[arg(required = true, num_args = 1.., add = archived_pads_completer())]
        indexes: Vec<String>,
    },

    /// Pin one or more pads (makes them delete-protected)
    #[command(alias = "p", display_order = 17)]
    #[dispatch(pure, template = "modification_result")]
    Pin {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Unpin one or more pads
    #[command(alias = "u", display_order = 18)]
    #[dispatch(pure, template = "modification_result")]
    Unpin {
        /// Indexes of the pads (e.g. p1 p2 p3)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Move one or more pads to a new parent
    #[command(alias = "mv", display_order = 13)]
    #[dispatch(pure, handler = handlers::move_pads__handler, template = "modification_result")]
    Move {
        /// Indexes of the pads (e.g. 1 2)
        /// If --root is NOT specified, the last argument is the destination.
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,

        /// Move to the root level (detach from any parent)
        #[arg(long, short = 'r')]
        root: bool,
    },

    /// Print the file path to one or more pads
    #[command(display_order = 17)]
    #[dispatch(pure, template = "path")]
    Path {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = all_pads_completer())]
        indexes: Vec<String>,
    },

    /// Print the UUID of one or more pads
    #[command(display_order = 17)]
    #[dispatch(pure, template = "uuid")]
    Uuid {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = all_pads_completer())]
        indexes: Vec<String>,
    },

    /// Mark pads as done (completed)
    #[command(alias = "done", display_order = 18)]
    #[dispatch(pure, template = "modification_result")]
    Complete {
        /// Indexes of the pads (e.g. 1 3 5 or 1-5)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Reopen pads (set back to planned)
    #[command(display_order = 19)]
    #[dispatch(pure, template = "modification_result")]
    Reopen {
        /// Indexes of the pads (e.g. 1 3 5 or 1-5)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    // --- Data operations ---
    /// Permanently delete pads
    #[command(display_order = 20)]
    #[dispatch(pure, template = "purge")]
    Purge {
        /// Indexes of the pads (e.g. d1 d2) - if omitted, purges all deleted pads
        #[arg(required = false, num_args = 0.., add = deleted_pads_completer())]
        indexes: Vec<String>,

        /// Skip confirmation
        #[arg(long, short = 'y')]
        yes: bool,

        /// Required when purging pads that have children (will purge entire subtree)
        #[arg(long, short = 'r')]
        recursive: bool,
    },

    /// Export pads to a tar.gz archive (or single file with --single-file)
    #[command(display_order = 21)]
    #[dispatch(pure, template = "export")]
    Export {
        /// Export all pads to a single file with this title (format detected from extension: .md for markdown, otherwise text)
        #[arg(long, value_name = "TITLE", conflicts_with = "json")]
        single_file: Option<String>,

        /// Export as a JSON archive (.tar.gz) preserving full metadata:
        /// timestamps, status, pinning, tags, and parent relationships.
        /// Round-trippable via `padz import`.
        #[arg(long, conflicts_with_all = ["single_file", "with_metadata"])]
        json: bool,

        /// Embed metadata inline in each exported file (md frontmatter /
        /// lex annotations). Exported files use each pad's native extension;
        /// .txt pads are exported without metadata (txt has no metadata
        /// format) and listed in a trailing warning.
        #[arg(long = "with-metadata", conflicts_with_all = ["single_file", "json"])]
        with_metadata: bool,

        /// Indexes of the pads (e.g. 1 2) - if omitted, exports all active pads
        #[arg(required = false, num_args = 0.., add = active_pads_completer())]
        indexes: Vec<String>,

        /// Show only the selected pad(s), no children
        #[arg(long, conflicts_with_all = ["tree", "indented"])]
        flat: bool,

        /// Recursively include children (default)
        #[arg(long, conflicts_with_all = ["flat", "indented"])]
        tree: bool,

        /// Recursively include children with 4-space indentation per level
        #[arg(long, conflicts_with_all = ["flat", "tree"])]
        indented: bool,
    },

    /// Import files as pads
    #[command(display_order = 22)]
    #[dispatch(pure, template = "import")]
    Import {
        /// Paths to files or directories to import
        #[arg(required = true, num_args = 1..)]
        paths: Vec<String>,
    },

    /// Copy pads to (or from) another padz store (source is kept)
    #[command(display_order = 23)]
    #[dispatch(pure, template = "transfer")]
    Clone {
        /// Indexes of the pads (e.g. 1 2) - if omitted, all non-deleted pads (active + archived)
        #[arg(required = false, num_args = 0.., add = active_pads_completer())]
        indexes: Vec<String>,

        /// Copy pads from the current store into <PATH>
        #[arg(long, value_name = "PATH", conflicts_with = "from")]
        to: Option<String>,

        /// Copy pads from <PATH> into the current store
        #[arg(long, value_name = "PATH", conflicts_with = "to")]
        from: Option<String>,
    },

    /// Move pads to (or from) another padz store (source is removed on success)
    #[command(display_order = 24)]
    #[dispatch(pure, template = "transfer")]
    Migrate {
        /// Indexes of the pads (e.g. 1 2) - if omitted, all non-deleted pads (active + archived)
        #[arg(required = false, num_args = 0.., add = active_pads_completer())]
        indexes: Vec<String>,

        /// Move pads from the current store into <PATH>
        #[arg(long, value_name = "PATH", conflicts_with = "from")]
        to: Option<String>,

        /// Move pads from <PATH> into the current store
        #[arg(long, value_name = "PATH", conflicts_with = "to")]
        from: Option<String>,
    },

    // --- Tags (nested subcommand) ---
    /// Manage tags
    #[command(subcommand, display_order = 25)]
    #[dispatch(nested)]
    Tag(TagCommands),

    // --- Misc commands ---
    /// Check and fix data inconsistencies
    #[command(display_order = 30)]
    #[dispatch(pure, template = "doctor")]
    Doctor,

    /// Manage configuration
    #[command(display_order = 31)]
    #[dispatch(skip)]
    Config {
        #[command(subcommand)]
        action: Option<ConfigSubcommand>,
    },

    /// Initialize the store (optional utility)
    #[command(display_order = 32)]
    #[dispatch(pure, template = "initialization")]
    Init {
        /// Link to another project's padz data
        #[arg(long, value_name = "PATH", conflicts_with = "unlink")]
        link: Option<String>,

        /// Remove an existing link
        #[arg(long, conflicts_with = "link")]
        unlink: bool,
    },

    /// Shell completion setup
    #[command(display_order = 34, name = "completion")]
    #[dispatch(skip)]
    Completion {
        /// Shell to target (auto-detected from $SHELL if omitted)
        #[arg(long, short, value_enum)]
        shell: Option<CompletionShell>,

        #[command(subcommand)]
        action: CompletionAction,
    },
}

/// Configuration subcommands (mirrors clapfig::ConfigSubcommand but avoids
/// --output collision with standout's global --output flag).
#[derive(Subcommand, Debug)]
pub enum ConfigSubcommand {
    /// Show all resolved configuration values
    List,
    /// Generate a commented sample padz.toml
    Gen {
        /// Write to a file instead of stdout
        #[arg(short = 'o', long = "out")]
        file: Option<std::path::PathBuf>,
    },
    /// Show the resolved value for a config key
    Get {
        /// Dotted key path (e.g. "format")
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Dotted key path (e.g. "format")
        key: String,
        /// Value to set
        value: String,
    },
}

/// Tag subcommands
#[derive(Subcommand, Dispatch, Debug)]
#[dispatch(handlers = handlers::tag)]
pub enum TagCommands {
    /// Add tags to pads (auto-creates tags if needed)
    #[command(display_order = 25)]
    #[dispatch(pure, template = "tagging")]
    Add {
        /// Pad selectors followed by tag names (e.g. 1 2 feature work)
        #[arg(required = true, num_args = 1..)]
        args: Vec<String>,
    },

    /// Remove tags from pads
    #[command(display_order = 26)]
    #[dispatch(pure, template = "tagging")]
    Remove {
        /// Pad selectors followed by tag names (e.g. 1 2 feature work)
        #[arg(required = true, num_args = 1..)]
        args: Vec<String>,
    },

    /// Rename a tag (updates all pads)
    #[command(alias = "mv", display_order = 27)]
    #[dispatch(pure, template = "tag_registry")]
    Rename {
        /// Current name of the tag
        old_name: String,
        /// New name for the tag
        new_name: String,
    },

    /// Delete a tag (removes from all pads)
    #[command(alias = "rm", display_order = 28)]
    #[dispatch(pure, template = "tag_registry")]
    Delete {
        /// Name of the tag to delete
        name: String,
    },

    /// List all defined tags, or tags for specific pads
    #[command(alias = "ls", display_order = 29)]
    #[dispatch(pure, template = "tag_catalog")]
    List {
        /// Pad IDs to show tags for (e.g. 1, 2 3, 1-3). If omitted, lists all tags.
        #[arg(num_args = 0..)]
        ids: Vec<String>,
    },
}

/// Completion subcommands
#[derive(Subcommand, Debug)]
pub enum CompletionAction {
    /// Install completion script to the standard location
    Install,

    /// Print completion script to stdout
    Print,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use standout::cli::validate_command_groups;

    #[test]
    fn test_completion_install_no_shell() {
        let cli = Cli::try_parse_from(["padz", "completion", "install"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Completion {
                shell: None,
                action: CompletionAction::Install,
            })
        ));
    }

    #[test]
    fn test_completion_install_with_shell() {
        let cli =
            Cli::try_parse_from(["padz", "completion", "--shell", "bash", "install"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Completion {
                shell: Some(CompletionShell::Bash),
                action: CompletionAction::Install,
            })
        ));
    }

    #[test]
    fn test_completion_print() {
        let cli = Cli::try_parse_from(["padz", "completion", "print"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Completion {
                shell: None,
                action: CompletionAction::Print,
            })
        ));
    }

    #[test]
    fn test_completion_print_with_shell() {
        let cli = Cli::try_parse_from(["padz", "completion", "--shell", "zsh", "print"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Completion {
                shell: Some(CompletionShell::Zsh),
                action: CompletionAction::Print,
            })
        ));
    }

    #[test]
    fn test_completion_install_fish() {
        let cli =
            Cli::try_parse_from(["padz", "completion", "--shell", "fish", "install"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Completion {
                shell: Some(CompletionShell::Fish),
                action: CompletionAction::Install,
            })
        ));
    }

    #[test]
    fn test_help_groups_match_commands() {
        // Use augmented command because standout adds the `help` subcommand
        let app = app_with_topics();
        let cmd = app.augment_command_with_help(Cli::command());
        validate_command_groups(&cmd, &command_groups()).unwrap();
    }

    #[test]
    fn test_data_option_parses() {
        let cli = Cli::try_parse_from(["padz", "--data", "/path/to/.padz", "list"]).unwrap();
        assert_eq!(cli.data, Some("/path/to/.padz".to_string()));
        assert!(!cli.global);
    }

    #[test]
    fn test_data_option_with_equals() {
        let cli = Cli::try_parse_from(["padz", "--data=/custom/data", "list"]).unwrap();
        assert_eq!(cli.data, Some("/custom/data".to_string()));
    }

    #[test]
    fn test_data_option_before_command() {
        let cli = Cli::try_parse_from(["padz", "--data", "/tmp/.padz", "create", "test"]).unwrap();
        assert_eq!(cli.data, Some("/tmp/.padz".to_string()));
        assert!(matches!(cli.command, Some(Commands::Create { .. })));
    }

    #[test]
    fn test_data_option_after_command() {
        // Global options can appear after subcommand
        let cli = Cli::try_parse_from(["padz", "list", "--data", "/tmp/.padz"]).unwrap();
        assert_eq!(cli.data, Some("/tmp/.padz".to_string()));
    }

    #[test]
    fn test_data_and_global_options_conflict() {
        // --data and -g are mutually exclusive
        let result = Cli::try_parse_from(["padz", "--data", "/tmp/.padz", "-g", "list"]);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("--data") || err.contains("--global"));
    }

    #[test]
    fn test_no_data_option() {
        let cli = Cli::try_parse_from(["padz", "list"]).unwrap();
        assert_eq!(cli.data, None);
    }

    #[test]
    fn test_data_option_with_worktree_path() {
        // Real-world use case: git worktree pointing to main repo's .padz
        let cli = Cli::try_parse_from([
            "padz",
            "--data",
            "/home/user/project/.padz",
            "create",
            "todo",
        ])
        .unwrap();
        assert_eq!(cli.data, Some("/home/user/project/.padz".to_string()));
    }
}
