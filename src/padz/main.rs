//! # Padz CLI Architecture
//!
//! Padz ships with a fully fledged CLI client, but the binary is intentionally thin:
//! the CLI lives in `src/padz/cli`, while this file only invokes `cli::run()` and
//! handles process termination. The CLI itself is organized to keep the
//! UI-specific concerns **entirely separate** from the application logic.
//!
//! ## Layering
//!
//! The overall architecture mirrors the library docs, but from the CLI vantage point:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  CLI Layer (src/padz/cli)                                   │
//! │  - clap argument parsing (setup.rs)                         │
//! │  - Command selection + context wiring (commands.rs)         │
//! │  - Terminal rendering via Outstanding templates (render.rs) │
//! │  - Shell completion scripts + helpers                       │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  API Layer (src/padz/api.rs)                                │
//! │  - Normalizes user-facing IDs → UUIDs                       │
//! │  - Dispatches to command modules                            │
//! │  - Returns structured `CmdResult` values                    │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Command Layer (src/padz/commands/*)                        │
//! │  - Pure business logic + data access                        │
//! │  - No knowledge of stdout/stderr or process exits           │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! Everything from `api.rs` inward is UI agnostic: functions take normal Rust
//! values, return normal Rust types, and never assume terminal I/O. The CLI layer
//! is therefore responsible for **all** user-facing concerns: argument parsing,
//! context initialization, dispatch, error handling, and rendering.
//!
//! ## Rendering with Outstanding
//!
//! Terminal output is produced through the `outstanding` crate. Templates live in
//! `src/padz/cli/templates/` (e.g., `list.tmp`, `full_pad.tmp`) and are embedded at
//! compile time via `include_str!()`. `render.rs` feeds data structures into those
//! templates and the CLI commands simply print the rendered strings. This keeps CLI
//! layout changes isolated to template files while still producing self-contained
//! binaries.
//!
//! ## Testing Approach
//!
//! - **Commands layer (`src/padz/commands`)**: heavy unit testing of the business
//!   logic.
//! - **API layer (`src/padz/api.rs`)**: mock-focused tests to ensure the correct
//!   command functions are invoked with the right arguments and that results are
//!   normalized properly.
//! - **CLI layer (`src/padz/cli`)**: tests build `clap` argument strings, mock the
//!   API facade, and verify the CLI invokes API methods correctly. Rendering is
//!   verified by supplying canned `CmdResult` structs and comparing the template
//!   output.
//!
//! Development flows **inside-out**: implement and test command logic, expose it
//! via the API facade, and only then wire up CLI parsing + rendering.

mod cli;

fn main() {
    if let Err(e) = cli::run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
