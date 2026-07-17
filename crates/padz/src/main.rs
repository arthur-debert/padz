//! The `padz` binary.
//!
//! Deliberately thin: it wires the process boundary (shell completions, stderr,
//! exit codes) to `padz::cli::run()` and holds nothing else. The architecture
//! this shim sits on top of — and the test pyramid that proves it — is
//! documented on the library crate; see [`padz`].

use padz::cli;

fn main() {
    // Handle shell completions before normal CLI processing.
    // When COMPLETE=<shell> is set, this intercepts the request and exits.
    clap_complete::CompleteEnv::with_factory(cli::setup::build_command).complete();

    if let Err(e) = cli::run() {
        // `cli::errors::render` styles the errors that carry structured data
        // (see `padzapp::error::PadzError::AmbiguousTitle`) and falls back to
        // `Display` for the rest.
        eprintln!("Error: {}", cli::errors::render(&e));
        std::process::exit(1);
    }
}
