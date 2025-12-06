use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "pa")]
#[command(about = "Context-aware command-line note-taking tool", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Operate on global pads
    #[arg(short, long, global = true)]
    pub global: bool,

    /// Verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,
    // Catch-all for "naked" args (e.g. `padz 1` or `padz "note"`)
    // Clap has `external_subcommand` or just a trailing arg.
    // PADZ.md says: `padz 1` -> view, `padz "note"` -> create.
    // Complicated to map directly with simple subcommands.
    // Strategy: Use a default subcommand or handle parsing manually if Clap fails?
    // Or use `#[arg(trailing_var_arg = true)]`?
    // Let's stick to EXPLICIT subcommands first in this iteration.
}

#[derive(Subcommand, Debug)]
pub enum Commands {
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

    /// Search pads (dedicated command)
    Search { term: String },

    /// Print the file path to one or more pads
    Path {
        /// Indexes of the pads (e.g. 1 p1 d1)
        #[arg(required = true, num_args = 1..)]
        indexes: Vec<String>,
    },

    /// Get or set configuration
    Config {
        /// Configuration key (e.g., file-ext)
        key: Option<String>,

        /// Value to set (if omitted, prints current value)
        value: Option<String>,
    },

    /// Initialize the store (optional utility)
    Init,
}
