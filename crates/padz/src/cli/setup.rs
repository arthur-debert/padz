use super::complete::{active_pads_completer, all_pads_completer, deleted_pads_completer};
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand, ValueEnum};
use once_cell::sync::Lazy;
use standout::cli::{render_help_with_topics, App};
use standout::topics::TopicRegistry;
use standout::OutputMode;

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
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
    after_help = "Enable shell completions:\n  eval \"$(padz completions bash)\"  # add to ~/.bashrc\n  eval \"$(padz completions zsh)\"   # add to ~/.zshrc"
)]
#[command(about = "Context-aware command-line note-taking tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Operate on global pads
    #[arg(
        short,
        long,
        global = true,
        help_heading = "Options",
        conflicts_with = "data"
    )]
    pub global: bool,

    /// Verbose output
    #[arg(short, long, global = true, help_heading = "Options")]
    pub verbose: bool,

    /// Override data directory path (e.g., for git worktrees)
    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help_heading = "Options",
        conflicts_with = "global"
    )]
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
/// It also adds the --output flag for output mode control (auto, term, text, term-debug).
/// Returns the parsed CLI and the output mode extracted from the matches.
pub fn parse_cli() -> (Cli, OutputMode) {
    let app = App::with_registry(HELP_TOPICS.clone());
    let matches = app.parse_with(Cli::command());

    // Extract output mode from the matches (standout adds this as _output_mode)
    let output_mode = match matches
        .get_one::<String>("_output_mode")
        .map(|s| s.as_str())
    {
        Some("term") => OutputMode::Term,
        Some("text") => OutputMode::Text,
        Some("term-debug") => OutputMode::TermDebug,
        Some("json") => OutputMode::Json,
        _ => OutputMode::Auto,
    };

    let cli = Cli::from_arg_matches(&matches).expect("Failed to parse CLI arguments");
    (cli, output_mode)
}

/// Returns the help output as a styled string (used for empty list display).
pub fn get_grouped_help() -> String {
    let cmd = Cli::command();
    render_help_with_topics(&cmd, &HELP_TOPICS, None).unwrap_or_else(|_| {
        let version = cmd.get_version().unwrap_or("unknown");
        format!("padz {version}\nContext-aware command-line note-taking tool\n\nUsage: padz [OPTIONS] [COMMAND]\n")
    })
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(flatten)]
    Core(CoreCommands),

    #[command(flatten)]
    Pad(PadCommands),

    #[command(flatten)]
    Data(DataCommands),

    /// Manage tags
    #[command(subcommand, display_order = 25)]
    Tags(TagsCommands),

    #[command(flatten)]
    Misc(MiscCommands),
}

#[derive(Subcommand, Debug)]
pub enum CoreCommands {
    /// Create a new pad
    #[command(alias = "n", display_order = 1)]
    Create {
        /// Skip opening the editor
        #[arg(long)]
        no_editor: bool,

        /// Create inside another pad (parent selector, e.g. 1 or p1)
        #[arg(long, short = 'i')]
        inside: Option<String>,

        /// Title words (joined with spaces, optional - opens empty editor if not provided)
        #[arg(trailing_var_arg = true)]
        title: Vec<String>,
    },

    /// List pads
    #[command(alias = "ls", display_order = 2)]
    List {
        /// Search term
        #[arg(short, long)]
        search: Option<String>,

        /// Show deleted pads
        #[arg(long)]
        deleted: bool,

        /// Peek at pad content
        #[arg(long)]
        peek: bool,

        /// Show only planned pads
        #[arg(long, conflicts_with_all = ["done", "in_progress"])]
        planned: bool,

        /// Show only done pads
        #[arg(long, conflicts_with_all = ["planned", "in_progress"])]
        done: bool,

        /// Show only in-progress pads
        #[arg(long, conflicts_with_all = ["planned", "done"])]
        in_progress: bool,

        /// Filter by tag(s) (can be specified multiple times, uses AND logic)
        #[arg(long = "tag", short = 't', num_args = 1..)]
        tags: Vec<String>,
    },

    /// Search pads (dedicated command)
    #[command(display_order = 3)]
    Search {
        /// Search term
        term: String,

        /// Filter by tag(s) (can be specified multiple times, uses AND logic)
        #[arg(long = "tag", short = 't', num_args = 1..)]
        tags: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum PadCommands {
    /// View one or more pads
    #[command(alias = "v", display_order = 10)]
    View {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = all_pads_completer())]
        indexes: Vec<String>,

        /// Peek at pad content
        #[arg(long)]
        peek: bool,
    },

    /// Edit a pad in the editor
    #[command(alias = "e", display_order = 11)]
    Edit {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Open a pad in the editor (copies to clipboard on exit)
    #[command(alias = "o", display_order = 12)]
    Open {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = all_pads_completer())]
        indexes: Vec<String>,
    },

    /// Delete one or more pads (protected pads must be unpinned first)
    #[command(alias = "rm", display_order = 13)]
    Delete {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(num_args = 1.., add = active_pads_completer(), required_unless_present = "done_status")]
        indexes: Vec<String>,

        /// Delete all pads marked as done
        #[arg(long = "done", conflicts_with = "indexes")]
        done_status: bool,
    },

    /// Restore deleted pads
    #[command(display_order = 14)]
    Restore {
        /// Indexes of deleted pads (e.g. d1 d2 or just 1 2)
        #[arg(required = true, num_args = 1.., add = deleted_pads_completer())]
        indexes: Vec<String>,
    },

    /// Pin one or more pads (makes them delete-protected)
    #[command(alias = "p", display_order = 15)]
    Pin {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Unpin one or more pads
    #[command(alias = "u", display_order = 16)]
    Unpin {
        /// Indexes of the pads (e.g. p1 p2 p3)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Move one or more pads to a new parent
    #[command(alias = "mv", display_order = 13)]
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
    Path {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1.., add = all_pads_completer())]
        indexes: Vec<String>,
    },

    /// Mark pads as done (completed)
    #[command(alias = "done", display_order = 18)]
    Complete {
        /// Indexes of the pads (e.g. 1 3 5 or 1-5)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Reopen pads (set back to planned)
    #[command(display_order = 19)]
    Reopen {
        /// Indexes of the pads (e.g. 1 3 5 or 1-5)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Add tags to pads
    #[command(name = "add-tag", display_order = 23)]
    AddTag {
        /// Indexes of the pads (e.g. 1 3 5 or 1-5)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,

        /// Tag(s) to add (can be specified multiple times)
        #[arg(long = "tag", short = 't', required = true, num_args = 1..)]
        tags: Vec<String>,
    },

    /// Remove tags from pads
    #[command(name = "remove-tag", display_order = 24)]
    RemoveTag {
        /// Indexes of the pads (e.g. 1 3 5 or 1-5)
        #[arg(required = true, num_args = 1.., add = active_pads_completer())]
        indexes: Vec<String>,

        /// Tag(s) to remove (can be specified multiple times) - if omitted, clears all tags
        #[arg(long = "tag", short = 't', num_args = 1..)]
        tags: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DataCommands {
    /// Permanently delete pads
    #[command(display_order = 20)]
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
    Export {
        /// Export all pads to a single file with this title (format detected from extension: .md for markdown, otherwise text)
        #[arg(long, value_name = "TITLE")]
        single_file: Option<String>,

        /// Indexes of the pads (e.g. 1 2) - if omitted, exports all active pads
        #[arg(required = false, num_args = 0.., add = active_pads_completer())]
        indexes: Vec<String>,
    },

    /// Import files as pads
    #[command(display_order = 22)]
    Import {
        /// Paths to files or directories to import
        #[arg(required = true, num_args = 1..)]
        paths: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum TagsCommands {
    /// List all defined tags
    #[command(alias = "ls", display_order = 25)]
    List,

    /// Create a new tag
    #[command(display_order = 26)]
    Create {
        /// Name of the tag to create
        name: String,
    },

    /// Delete a tag (removes from all pads)
    #[command(alias = "rm", display_order = 27)]
    Delete {
        /// Name of the tag to delete
        name: String,
    },

    /// Rename a tag (updates all pads)
    #[command(alias = "mv", display_order = 28)]
    Rename {
        /// Current name of the tag
        old_name: String,
        /// New name for the tag
        new_name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum MiscCommands {
    /// Check and fix data inconsistencies
    #[command(display_order = 30)]
    Doctor,

    /// Get or set configuration
    #[command(display_order = 31)]
    Config {
        /// Configuration key (e.g., file-ext)
        key: Option<String>,

        /// Value to set (if omitted, prints current value)
        value: Option<String>,
    },

    /// Initialize the store (optional utility)
    #[command(display_order = 32)]
    Init,

    /// Generate shell completions
    #[command(display_order = 34)]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: CompletionShell,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

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
        assert!(matches!(
            cli.command,
            Some(Commands::Core(CoreCommands::Create { .. }))
        ));
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
