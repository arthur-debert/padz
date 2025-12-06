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

    /// View a pad
    #[command(alias = "v")]
    View {
        /// Index of the pad (e.g. 1, p1, d1)
        index: String,
    },

    /// Edit a pad in the editor
    #[command(alias = "e")]
    Edit {
        /// Index of the pad (e.g. 1, p1, d1)
        index: String,
    },

    /// Open a pad in the editor (copies to clipboard on exit)
    #[command(alias = "o")]
    Open {
        /// Index of the pad (e.g. 1, p1, d1)
        index: String,
    },

    /// Delete a pad
    #[command(alias = "rm")]
    Delete { index: String },

    /// Pin a pad
    #[command(alias = "p")]
    Pin { index: String },

    /// Unpin a pad
    #[command(alias = "u")]
    Unpin { index: String },

    /// Search pads (dedicated command)
    Search { term: String },

    /// Print the file path to a pad
    Path {
        /// Index of the pad (e.g. 1, p1, d1)
        index: String,
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
