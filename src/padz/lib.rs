//! # Padz Architecture: CLI / Logic Separation
//!
//! Padz is a **UI-agnostic note-taking library**. This is not a CLI application that happens
//! to have some library code—it's a library that happens to have a CLI client.
//!
//! ## The Problem
//!
//! Shell programs tend to conflate interface and logic. Before you know it, logic is outputting
//! strings all over the place and calculations are converting strings to integers. This makes
//! the program unwieldy and hard to test.
//!
//! ## The Solution: Strict Layering
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  CLI Layer (cli/)                                           │
//! │  - Parses arguments, formats output, handles terminal I/O   │
//! │  - The ONLY place that knows about stdout/stderr/exit codes │
//! └─────────────────────────────────────────────────────────────┘
//!                              │
//!                              ▼
//! ┌─────────────────────────────────────────────────────────────┐
//! │  API Layer (api.rs)                                         │
//! │  - Thin facade over commands                                │
//! │  - Normalizes inputs (indexes → UUIDs)                      │
//! │  - Returns structured Result<CmdResult>                     │
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
//! ## Key Principle: No I/O Assumptions in Core
//!
//! From `api.rs` inward, code:
//! - Takes regular Rust function arguments
//! - Returns regular Rust types (`Result<CmdResult>`)
//! - **Never** writes to stdout/stderr
//! - **Never** calls `std::process::exit`
//! - **Never** assumes a terminal environment
//!
//! This means the same core could serve a REST API, a browser app, or any other UI.
//!
//! ## The Index System
//!
//! To remain ergonomic, padz uses a dual ID system mapping user-friendly display indexes
//! (`1`, `p1`, `d1`) to stable UUIDs at the data store level.
//! See [`index`] module for details.
//!
//! ## Data Flow Example: `padz delete 1`
//!
//! 1. **CLI**: `clap` parses `1`. `handle_delete` receives `vec!["1"]`.
//! 2. **API**: `parse_selectors` maps `"1"` → `PadSelector::Index(Regular(1))`.
//! 3. **Command**: Resolves to UUID, checks protection, marks as deleted.
//! 4. **Return**: `CmdResult` with message "Deleted 1 pad".
//! 5. **CLI**: Renders the message to terminal.
//!
//! ## Testing Strategy
//!
//! | Layer | Test Type | What to Test |
//! |-------|-----------|--------------|
//! | CLI | Integration | Arg parsing, output formatting |
//! | API | Unit | Dispatch correctness, input normalization |
//! | Commands | Unit | Business logic, state transitions |
//! | Store | Unit | Read/write operations, sync behavior |
//!
//! ## Development Workflow
//!
//! When implementing features, work **inside-out**:
//!
//! 1. **Logic**: Implement and fully test in `commands/<cmd>.rs`
//! 2. **API**: Add facade method in `api.rs`, test dispatch
//! 3. **CLI**: Add handler in `cli/commands.rs`, test arg parsing and output
//!
//! ## Module Overview
//!
//! - [`api`]: The API facade—entry point for all operations. Also handles selector parsing.
//! - [`commands`]: Business logic for each command
//! - [`store`]: Storage abstraction and implementations. See module for hybrid store architecture.
//! - [`model`]: Core data types and content normalization
//! - [`index`]: Display indexing system (p1, 1, d1 notation)
//! - [`init`]: Scope detection and context initialization
//! - [`config`]: Configuration management
//! - [`editor`]: External editor integration
//! - [`clipboard`]: Cross-platform clipboard support
//! - [`error`]: Error types
//! - `cli`: CLI behavior, argument parsing, and output formatting (not part of lib API)

pub mod api;
pub mod clipboard;
pub mod commands;
pub mod config;
pub mod editor;
pub mod error;
pub mod index;
pub mod init;
pub mod model;
pub mod peek;
pub mod store;
