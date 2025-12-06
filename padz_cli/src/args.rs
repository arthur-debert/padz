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
        /// Title of the pad
        #[arg(required = true)]
        title: String,
        
        /// Content of the pad (optional, reads from stdin if piped?)
        /// For now just an arg.
        #[arg(required = false)]
        content: Option<String>,
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
    
    /// Delete a pad
    #[command(alias = "rm")]
    Delete {
        index: String,
    },

    /// Pin a pad
    #[command(alias = "p")]
    Pin {
        index: String,
    },
    
    /// Unpin a pad
    #[command(alias = "u")]
    Unpin {
        index: String,
    },
    
    /// Search pads (dedicated command)
    Search {
        term: String,
    },

    /// Initialize the store (optional utility)
    Init,
}
