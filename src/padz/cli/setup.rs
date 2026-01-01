use clap::{CommandFactory, Parser, Subcommand, ValueEnum};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
}

/// Returns the version string, including git hash and commit date for non-release builds.
/// Format: "0.8.10" for releases, "0.8.10@abc1234 2024-01-15 14:30" for dev builds
fn get_version() -> &'static str {
    const VERSION: &str = env!("CARGO_PKG_VERSION");
    const GIT_HASH: &str = env!("GIT_HASH");
    const GIT_COMMIT_DATE: &str = env!("GIT_COMMIT_DATE");
    const IS_RELEASE: &str = env!("IS_RELEASE");

    // Use a static to compute the version string once
    use std::sync::OnceLock;
    static VERSION_STRING: OnceLock<String> = OnceLock::new();

    VERSION_STRING.get_or_init(|| {
        if IS_RELEASE == "true" || GIT_HASH.is_empty() {
            VERSION.to_string()
        } else {
            format!("{}@{} {}", VERSION, GIT_HASH, GIT_COMMIT_DATE)
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
            "view" | "edit" | "open" | "delete" | "pin" | "unpin" | "path" => {
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

/// Returns the custom grouped help output as a string
pub fn get_grouped_help() -> String {
    let cmd = Cli::command();
    let version = cmd.get_version().unwrap_or("unknown");

    let mut output = String::new();
    output.push_str(&format!("padz {version}\n"));
    output.push_str("Context-aware command-line note-taking tool\n");
    output.push('\n');
    output.push_str("Usage: padz [OPTIONS] [COMMAND]\n");

    // Collect subcommands into groups
    let subcommands: Vec<_> = cmd.get_subcommands().collect();

    for group in CommandGroup::all() {
        let group_cmds: Vec<_> = subcommands
            .iter()
            .filter(|sc| {
                !sc.is_hide_set() && CommandGroup::for_command(sc.get_name()) == Some(*group)
            })
            .collect();

        if !group_cmds.is_empty() {
            output.push('\n');
            output.push_str(&format!("{}\n", group.heading()));
            for sc in group_cmds {
                let name = sc.get_name();
                let about = sc.get_about().map(|s| s.to_string()).unwrap_or_default();
                output.push_str(&format!("  {:<12} {}\n", name, about));
            }
        }
    }

    output.push('\n');
    output.push_str("Options:\n");
    output.push_str("  -g, --global     Operate on global pads\n");
    output.push_str("  -v, --verbose    Verbose output\n");
    output.push_str("  -h, --help       Print help\n");
    output.push_str("  -V, --version    Print version\n");

    output
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

/// Prints help for a command by name
pub fn print_help_for_command(name: &str) {
    let mut cmd = Cli::command();

    // Find and print help for the subcommand
    for subcmd in cmd.get_subcommands_mut() {
        if subcmd.get_name() == name {
            let help = subcmd.render_help();
            print!("{}", help);
            return;
        }
    }

    // Fallback to grouped help if subcommand not found
    eprintln!("Unknown command: {}", name);
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

    /// Delete one or more pads
    #[command(alias = "rm", display_order = 13)]
    Delete {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Pin one or more pads
    #[command(alias = "p", display_order = 14)]
    Pin {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Unpin one or more pads
    #[command(alias = "u", display_order = 15)]
    Unpin {
        /// Indexes of the pads (e.g. p1 p2 p3)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Print the file path to one or more pads
    #[command(display_order = 16)]
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
