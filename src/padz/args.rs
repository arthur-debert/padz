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
    // --- Core Commands ---
    #[command(next_help_heading = "Core Commands")]
    /// Create a new pad
    #[command(alias = "n")]
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
    #[command(alias = "ls")]
    List {
        /// Search term
        #[arg(short, long)]
        search: Option<String>,

        /// Show deleted pads
        #[arg(long)]
        deleted: bool,
    },

    /// Search pads (dedicated command)
    Search { term: String },

    // --- For Each Pad ---
    #[command(next_help_heading = "For Each Pad")]
    /// View one or more pads
    #[command(alias = "v")]
    View {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Edit a pad in the editor
    #[command(alias = "e")]
    Edit {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Open a pad in the editor (copies to clipboard on exit)
    #[command(alias = "o")]
    Open {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Delete one or more pads
    #[command(alias = "rm")]
    Delete {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Pin one or more pads
    #[command(alias = "p")]
    Pin {
        /// Indexes of the pads (e.g. 1 3 5)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Unpin one or more pads
    #[command(alias = "u")]
    Unpin {
        /// Indexes of the pads (e.g. p1 p2 p3)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Print the file path to one or more pads
    Path {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    // --- Data ---
    #[command(next_help_heading = "Data")]
    /// Permanently delete pads
    Purge {
        /// Indexes of the pads (e.g. d1 d2) - if omitted, purges all deleted pads
        #[arg(required = false, num_args = 0..)]
        indexes: Vec<String>,

        /// Skip confirmation
        #[arg(long, short = 'y')]
        yes: bool,
    },
    // (Import, Export will go here)
    /// Export pads to a tar.gz archive
    Export {
        /// Indexes of the pads (e.g. 1 2) - if omitted, exports all active pads
        #[arg(required = false, num_args = 0..)]
        indexes: Vec<String>,
    },

    /// Import files as pads
    Import {
        /// Paths to files or directories to import
        #[arg(required = true, num_args = 1..)]
        paths: Vec<String>,
    },

    // --- Misc ---
    #[command(next_help_heading = "Misc")]
    /// Get or set configuration
    Config {
        /// Configuration key (e.g., file-ext)
        key: Option<String>,

        /// Value to set (if omitted, prints current value)
        value: Option<String>,
    },

    /// Initialize the store (optional utility)
    Init,

    /// Generate shell completions
    #[command(hide = true)]
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: CompletionShell,
    },

    /// Output pad indexes for shell completion (hidden)
    #[command(hide = true, name = "__complete-pads")]
    CompletePads {
        /// Include deleted pads
        #[arg(long)]
        deleted: bool,
    },
}
