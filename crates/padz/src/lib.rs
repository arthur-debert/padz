//! # Padz CLI Architecture
//!
//! Padz ships with a fully fledged CLI client, but the binary is intentionally thin:
//! the CLI lives in `src/cli/`, while `main.rs` only invokes `cli::run()` and
//! handles process termination. The CLI itself is organized to keep the
//! UI-specific concerns **entirely separate** from the application logic.
//!
//! ## Why this crate has a library target
//!
//! The binary is the *product*; this library target is the *seam*. Everything the
//! CLI is made of — the clap command tree, the typed handlers, the app builder —
//! is reachable from here, which is what lets tests exercise those pieces in
//! process instead of spawning `padz` and scraping its stdout. `main.rs` is a
//! seven-line shim over `cli::run()`; it holds no logic that tests would want.
//!
//! ## Workspace Structure
//!
//! Padz is organized as a Cargo workspace with two crates:
//! - `crates/padzapp/` — Core library with UI-agnostic business logic
//! - `crates/padz/` — This CLI tool, depends on the `padzapp` library
//!
//! ## Layering
//!
//! The overall architecture mirrors the library docs, but from the CLI vantage point:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  CLI Layer (crates/padz/src/cli/)                           │
//! │  - clap argument parsing (setup.rs)                         │
//! │  - Command selection + context wiring (commands.rs)         │
//! │  - Terminal rendering via Standout templates (render.rs)    │
//! │  - Shell completion scripts + helpers                       │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  API Layer (crates/padzapp/src/api/)                        │
//! │  - Normalizes user-facing IDs → UUIDs                       │
//! │  - Dispatches to command modules                            │
//! │  - Returns structured `CmdResult` values                    │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Command Layer (crates/padzapp/src/commands/*)              │
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
//! ## Rendering with Standout
//!
//! Terminal output is produced through the `standout` crate. Templates live in
//! `src/cli/templates/` (e.g., `list.jinja`, `full_pad.jinja`) and are embedded at
//! compile time via `embed_templates!()`. `render.rs` feeds data structures into
//! those templates and the CLI commands simply print the rendered strings. This
//! keeps CLI layout changes isolated to template files while still producing
//! self-contained binaries.
//!
//! ## Testing Approach — the Standout-shaped pyramid
//!
//! Each layer is proven at the **smallest seam that can observe the behavior**, so
//! a test fails for one reason and keeps failing for that reason across refactors:
//!
//! 1. **Direct `padzapp` tests** (`crates/padzapp/`) — domain behavior: validation,
//!    filtering, state transitions, persistence, result data. No CLI in sight.
//! 2. **Direct typed-handler tests** (`tests/handlers_direct.rs`) — the adapter
//!    mapping only: arguments in, API call, typed result out. These call
//!    [`cli::handlers`] functions with real Rust values, so no `ArgMatches` is
//!    constructed and no rendering happens.
//! 3. **`TestHarness` tests** (`tests/harness.rs`) — Clap-through-render
//!    integration: command wiring, input chains, templates, styles, output modes,
//!    output files. In process, and **serial**, because the seams they drive
//!    (env vars, cwd, terminal detectors, default readers) are process-global.
//! 4. **Subprocess E2E** (`tests/*_e2e.rs`) — only boundaries a harness cannot
//!    model: a real editor/clipboard process, completion installation, `main.rs`'s
//!    own wiring, or `std::process::exit` codes. Each retained file says which
//!    boundary it protects at the top.
//!
//! Development flows **inside-out**: implement and test command logic, expose it
//! via the API facade, and only then wire up CLI parsing + rendering.

pub mod cli;
