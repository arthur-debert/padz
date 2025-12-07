//! Binary entrypoint for the padz CLI.
//!
//! The CLI implementation now lives in the `cli/` module tree (see `src/padz/cli`).
//! This file simply delegates to that module and takes care of top-level process exits.

mod cli;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
