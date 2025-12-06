use clap::{Parser, Subcommand, ValueEnum};

#[derive(Copy, Clone, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
}

#[derive(Parser, Debug)]
#[command(name = "padz", bin_name = "padz", version)]
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
        /// Title of the pad (optional, opens editor if not provided)
        #[arg(required = false)]
        title: Option<String>,

        /// Content of the pad
        #[arg(required = false)]
        content: Option<String>,

        /// Skip opening the editor
        #[arg(long)]
        no_editor: bool,
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

    /// Export pads to a tar.gz archive
    #[command(display_order = 21)]
    Export {
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

    /// Generate shell completions
    #[command(hide = true, display_order = 33)]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: CompletionShell,
    },

    /// Output pad indexes for shell completion (hidden)
    #[command(hide = true, name = "__complete-pads", display_order = 34)]
    CompletePads {
        /// Shell to generate completions for
        #[arg(long)]
        deleted: bool,
    },
}
