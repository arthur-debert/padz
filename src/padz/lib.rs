//! # Padz Architecture
//!
//! Padz is a **UI-agnostic note-taking library**. This is not a CLI application that happens
//! to have some library code—it's a library that happens to have a CLI client.
//!
//! This distinction drives the entire architecture and should guide all development.
//!
//! ## The Three-Layer Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  CLI Layer (cli/, wired by main.rs)                         │
//! │  - Parses arguments, formats output, handles terminal I/O   │
//! │  - The ONLY place that knows about stdout/stderr/exit codes │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  API Layer (api.rs)                                         │
//! │  - Thin facade over commands                                │
//! │  - Normalizes inputs (indexes → UUIDs → Pads)               │
//! │  - Returns structured Result types                          │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Command Layer (commands/*.rs)                              │
//! │  - Pure business logic                                      │
//! │  - Operates on Rust types, returns Rust types               │
//! │  - No I/O assumptions whatsoever                            │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  Storage Layer (store/)                                     │
//! │  - Abstract DataStore trait                                 │
//! │  - FileStore (production), InMemoryStore (testing)          │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## The index System
//!    
//! In order to remanin erggnomic, Padz uses a dual id system that provides a mapping between user
//! friendly IDs (used throughout the cli) to the stable UUIDs at the data store level.
//! See index.rs for more information.
//!
//! ## Key Principle: No I/O Assumptions in Core
//!
//! From `api.rs` inward (API, commands, storage), code:
//! - Takes regular Rust function arguments
//! - Returns regular Rust types (`Result<CmdResult>`)
//! - **Never** writes to stdout/stderr
//! - **Never** calls `std::process::exit`
//! - **Never** assumes a terminal environment
//!
//! This means the same core could serve a REST API, a browser app, or any other UI.
//!
//! ## Testing Strategy
//!
//! The architecture enables focused testing at each layer:
//!
//! 1. **Commands** (`commands/*.rs`): Thorough unit tests of business logic.
//!    This is where the lion's share of testing lives.
//!
//! 2. **API** (`api.rs`): Mock tests verifying correct dispatch and return types.
//!    Tests that the right command is called with the right arguments—not the logic itself.
//!
//! 3. **CLI** (`cli/` + thin `main.rs`): Tests argument parsing and output formatting.
//!    - Input: craft shell argument strings, verify correct API calls
//!    - Output: given a `CmdResult`, verify correct terminal output
//!
//! ## Development Workflow
//!
//! When implementing features, work **inside-out**:
//!
//! 1. **Logic**: Implement and fully test in `commands/<cmd>.rs`
//! 2. **API**: Add facade method in `api.rs`, test dispatch
//! 3. **CLI**: Add handler in `main.rs`, test arg parsing and output
//!
//! ## Module Overview
//!
//! - [`api`]: The API facade—entry point for all operations
//! - [`commands`]: Business logic for each command
//! - [`store`]: Storage abstraction and implementations
//! - [`model`]: Core data types (`Pad`, `Metadata`, `Scope`)
//! - [`index`]: Display indexing system (p1, 1, d1 notation)
//! - [`config`]: Configuration management
//! - [`editor`]: External editor integration
//! - [`clipboard`]: Cross-platform clipboard support
//! - [`error`]: Error types
//! - `cli`: Argument parsing, printing, templated rendering, and shell completions for the binary (not part of the lib API)

pub mod api;
pub mod clipboard;
pub mod commands;
pub mod config;
pub mod editor;
pub mod error;
pub mod index;
pub mod model;
pub mod store;
