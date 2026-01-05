use crate::cli::styles::PADZ_THEME;
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use once_cell::sync::Lazy;
use outstanding::topics::TopicRegistry;
use outstanding::{render, ThemeChoice};
use outstanding_clap::{render_help as render_subcommand_help, render_topic, Config as HelpConfig};
use serde::Serialize;

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
    disable_help_flag = true,
    disable_help_subcommand = true
)]
#[command(about = "Context-aware command-line note-taking tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Operate on global pads
    #[arg(short, long, global = true, help_heading = "Options")]
    pub global: bool,

    /// Verbose output
    #[arg(short, long, global = true, help_heading = "Options")]
    pub verbose: bool,

    /// Print help
    #[arg(short, long, global = true)]
    pub help: bool,
}

/// Command group definitions for help output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandGroup {
    Core,
    Pad,
    Data,
    Misc,
}

impl CommandGroup {
    pub fn heading(&self) -> &'static str {
        match self {
            CommandGroup::Core => "Core Commands:",
            CommandGroup::Pad => "Per-Pad Commands:",
            CommandGroup::Data => "Data Commands:",
            CommandGroup::Misc => "Miscellaneous:",
        }
    }

    /// Returns the group for a given command name
    pub fn for_command(name: &str) -> Option<Self> {
        match name {
            "create" | "list" | "search" => Some(CommandGroup::Core),
            "view" | "edit" | "open" | "delete" | "restore" | "pin" | "unpin" | "path" => {
                Some(CommandGroup::Pad)
            }
            "purge" | "export" | "import" => Some(CommandGroup::Data),
            "doctor" | "config" | "init" | "help" => Some(CommandGroup::Misc),
            _ => None,
        }
    }

    /// Returns all groups in display order
    pub fn all() -> &'static [CommandGroup] {
        &[
            CommandGroup::Core,
            CommandGroup::Pad,
            CommandGroup::Data,
            CommandGroup::Misc,
        ]
    }
}

// Help template for grouped help output
const HELP_TEMPLATE: &str = include_str!("templates/help.tmp");
// Subcommand help template (fixes "Usage: Usage:" duplication)
const SUBCOMMAND_HELP_TEMPLATE: &str = include_str!("templates/subcommand_help.tmp");

// Help topics registry - loaded from topics directory
static HELP_TOPICS: Lazy<TopicRegistry> = Lazy::new(|| {
    let mut registry = TopicRegistry::new();
    // Topics are embedded at compile time from the topics directory
    // We manually add them since include_str! requires compile-time paths
    let topic_content = include_str!("topics/project-vs-global.txt");
    if let Some(topic) = parse_topic_file("project-vs-global", topic_content) {
        registry.add_topic(topic);
    }
    registry
});

/// Parse a topic file content into a Topic struct
fn parse_topic_file(name: &str, content: &str) -> Option<outstanding::topics::Topic> {
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

    Some(outstanding::topics::Topic::new(
        title,
        body,
        outstanding::topics::TopicType::Text,
        Some(name.to_string()),
    ))
}

/// Data structure for grouped help rendering
#[derive(Serialize)]
struct GroupedHelpData {
    about: String,
    usage: String,
    groups: Vec<HelpGroup>,
    options: Vec<HelpOption>,
}

#[derive(Serialize)]
struct HelpGroup {
    title: String,
    commands: Vec<HelpCommand>,
}

#[derive(Serialize)]
struct HelpCommand {
    name: String,
    about: String,
    padding: String,
}

#[derive(Serialize)]
struct HelpOption {
    name: String,
    help: String,
    padding: String,
}

/// Returns the short help output for -h flag (styled version)
pub fn get_short_help() -> String {
    get_grouped_help()
}

/// Prints the short help output
pub fn print_short_help() {
    print!("{}", get_short_help());
}

/// Returns the custom grouped help output as a styled string using outstanding
pub fn get_grouped_help() -> String {
    let cmd = Cli::command();
    let version = cmd.get_version().unwrap_or("unknown");

    // Collect subcommands into groups
    let subcommands: Vec<_> = cmd.get_subcommands().collect();
    let mut max_cmd_width = 0;

    // First pass: find max command name width
    for sc in &subcommands {
        if !sc.is_hide_set() {
            let name_len = sc.get_name().len();
            if name_len > max_cmd_width {
                max_cmd_width = name_len;
            }
        }
    }

    let mut groups = Vec::new();
    for group in CommandGroup::all() {
        let group_cmds: Vec<_> = subcommands
            .iter()
            .filter(|sc| {
                !sc.is_hide_set() && CommandGroup::for_command(sc.get_name()) == Some(*group)
            })
            .collect();

        if !group_cmds.is_empty() {
            let commands: Vec<HelpCommand> = group_cmds
                .iter()
                .map(|sc| {
                    let name = sc.get_name().to_string();
                    let pad = max_cmd_width.saturating_sub(name.len()) + 2;
                    HelpCommand {
                        name,
                        about: sc.get_about().map(|s| s.to_string()).unwrap_or_default(),
                        padding: " ".repeat(pad),
                    }
                })
                .collect();

            groups.push(HelpGroup {
                title: group.heading().to_string(),
                commands,
            });
        }
    }

    // Options with padding
    let options = vec![
        HelpOption {
            name: "-g, --global".to_string(),
            help: "Operate on global pads".to_string(),
            padding: "   ".to_string(),
        },
        HelpOption {
            name: "-v, --verbose".to_string(),
            help: "Verbose output".to_string(),
            padding: "  ".to_string(),
        },
        HelpOption {
            name: "-h, --help".to_string(),
            help: "Print help".to_string(),
            padding: "     ".to_string(),
        },
        HelpOption {
            name: "-V, --version".to_string(),
            help: "Print version".to_string(),
            padding: "  ".to_string(),
        },
    ];

    let data = GroupedHelpData {
        about: format!("padz {version}\nContext-aware command-line note-taking tool"),
        usage: "padz [OPTIONS] [COMMAND]".to_string(),
        groups,
        options,
    };

    render(HELP_TEMPLATE, &data, ThemeChoice::from(&*PADZ_THEME)).unwrap_or_else(|_| {
        // Fallback to plain text if rendering fails
        format!("padz {version}\nContext-aware command-line note-taking tool\n\nUsage: padz [OPTIONS] [COMMAND]\n")
    })
}

/// Generates the custom grouped help output
pub fn print_grouped_help() {
    print!("{}", get_grouped_help());
}

/// Prints help for a specific subcommand using clap's built-in rendering
pub fn print_subcommand_help(command: &Option<Commands>) {
    let subcommand_name = match command {
        Some(Commands::Core(c)) => match c {
            CoreCommands::Create { .. } => "create",
            CoreCommands::List { .. } => "list",
            CoreCommands::Search { .. } => "search",
        },
        Some(Commands::Pad(c)) => match c {
            PadCommands::View { .. } => "view",
            PadCommands::Edit { .. } => "edit",
            PadCommands::Open { .. } => "open",
            PadCommands::Delete { .. } => "delete",
            PadCommands::Restore { .. } => "restore",
            PadCommands::Pin { .. } => "pin",
            PadCommands::Unpin { .. } => "unpin",
            PadCommands::Path { .. } => "path",
        },
        Some(Commands::Data(c)) => match c {
            DataCommands::Purge { .. } => "purge",
            DataCommands::Export { .. } => "export",
            DataCommands::Import { .. } => "import",
        },
        Some(Commands::Misc(c)) => match c {
            MiscCommands::Doctor => "doctor",
            MiscCommands::Config { .. } => "config",
            MiscCommands::Init => "init",
            MiscCommands::Help { .. } => "help",
            MiscCommands::Completions { .. } => "completions",
            MiscCommands::CompletePads { .. } => "__complete-pads",
        },
        None => {
            print_grouped_help();
            return;
        }
    };

    print_help_for_command(subcommand_name);
}

/// Prints help for a command by name using outstanding-clap styled rendering
pub fn print_help_for_command(name: &str) {
    let cmd = Cli::command();

    // First, check if it's a subcommand
    for subcmd in cmd.get_subcommands() {
        if subcmd.get_name() == name {
            // Use outstanding-clap with custom template (fixes "Usage: Usage:" issue)
            let config = HelpConfig {
                template: Some(SUBCOMMAND_HELP_TEMPLATE.to_string()),
                ..Default::default()
            };
            match render_subcommand_help(subcmd, Some(config)) {
                Ok(help) => {
                    print!("{}", help);
                    return;
                }
                Err(_) => {
                    // Fallback to clap's default rendering if outstanding fails
                    let mut subcmd_clone = subcmd.clone();
                    let help = subcmd_clone.render_help();
                    print!("{}", help);
                    return;
                }
            }
        }
    }

    // Second, check if it's a help topic
    if let Some(topic) = HELP_TOPICS.get_topic(name) {
        match render_topic(topic, None) {
            Ok(help) => {
                print!("{}", help);
                return;
            }
            Err(_) => {
                // Fallback: print topic content directly
                println!("{}\n\n{}", topic.title, topic.content);
                return;
            }
        }
    }

    // Fallback to grouped help if neither subcommand nor topic found
    eprintln!("Unknown command or topic: {}", name);
    eprintln!();
    print_grouped_help();
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    #[command(flatten)]
    Core(CoreCommands),

    #[command(flatten)]
    Pad(PadCommands),

    #[command(flatten)]
    Data(DataCommands),

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
    },

    /// Search pads (dedicated command)
    #[command(display_order = 3)]
    Search { term: String },
}

#[derive(Subcommand, Debug)]
pub enum PadCommands {
    /// View one or more pads
    #[command(alias = "v", display_order = 10)]
    View {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,

        /// Peek at pad content
        #[arg(long)]
        peek: bool,
    },

    /// Edit a pad in the editor
    #[command(alias = "e", display_order = 11)]
    Edit {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Open a pad in the editor (copies to clipboard on exit)
    #[command(alias = "o", display_order = 12)]
    Open {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Delete one or more pads (protected pads must be unpinned first)
    #[command(alias = "rm", display_order = 13)]
    Delete {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Restore deleted pads
    #[command(display_order = 14)]
    Restore {
        /// Indexes of deleted pads (e.g. d1 d2 or just 1 2)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Pin one or more pads (makes them delete-protected)
    #[command(alias = "p", display_order = 15)]
    Pin {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Unpin one or more pads
    #[command(alias = "u", display_order = 16)]
    Unpin {
        /// Indexes of the pads (e.g. p1 p2 p3)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Print the file path to one or more pads
    #[command(display_order = 17)]
    Path {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DataCommands {
    /// Permanently delete pads
    #[command(display_order = 20)]
    Purge {
        /// Indexes of the pads (e.g. d1 d2) - if omitted, purges all deleted pads
        #[arg(required = false, num_args = 0..)]
        indexes: Vec<String>,

        /// Skip confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },

    /// Export pads to a tar.gz archive (or single file with --single-file)
    #[command(display_order = 21)]
    Export {
        /// Export all pads to a single file with this title (format detected from extension: .md for markdown, otherwise text)
        #[arg(long, value_name = "TITLE")]
        single_file: Option<String>,

        /// Indexes of the pads (e.g. 1 2) - if omitted, exports all active pads
        #[arg(required = false, num_args = 0..)]
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

    /// Print help for padz or a subcommand
    #[command(display_order = 33)]
    Help {
        /// Subcommand to get help for
        command: Option<String>,
    },

    /// Generate shell completions
    #[command(hide = true, display_order = 34)]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: CompletionShell,
    },

    /// Output pad indexes for shell completion (hidden)
    #[command(hide = true, name = "__complete-pads", display_order = 35)]
    CompletePads {
        /// Shell to generate completions for
        #[arg(long)]
        deleted: bool,
    },
}
