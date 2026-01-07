//! # CLI Behavior
//!
//! This is **one possible UI client** for padz—not the application itself.
//! The CLI is the only place that knows about terminal I/O, exit codes, and output formatting.
//!
//! For the overall architecture, see the crate-level documentation in [`crate`].
//!
//! ## Context-Aware Intelligence
//!
//! Padz infers intent from execution context (Arguments, Stdin, Clipboard).
//!
//! ### Naked Execution (`padz`)
//!
//! Running `padz` with no arguments defaults to `padz list`.
//! The "Read" operation is 90% of usage—it should be the path of least resistance.
//!
//! ### Smart Create (`padz create`)
//!
//! Priority order for content source:
//!
//! 1. **Piped Input** (highest priority)
//!    - `echo "foo" | padz create`
//!    - Creates pad with piped content. **Skips editor**.
//!
//! 2. **Clipboard** (fallback when no pipe and no title arg)
//!    - `padz create` (with something in clipboard)
//!    - Pre-fills editor with clipboard content. **Opens editor**.
//!
//! 3. **Title Argument**
//!    - `padz create "Meeting Notes"`
//!    - Uses argument as title. Opens editor for body.
//!
//! After saving from editor, pad content is automatically copied to clipboard.
//! Use `--no-editor` flag to skip opening the editor.
//!
//! ### View Copies to Clipboard
//!
//! `padz view 1` displays the pad AND copies its content to clipboard.
//! When viewing multiple pads, they are joined with `---` separators.
//!
//! ### Explicit Search
//!
//! - `padz search <term>` — Explicit search command.
//! - `padz list --search <term>` — Search within list.
//! - `padz view <term>` — If term isn't a valid index, treated as title search.
//!
//! **Design Choice**: Padz favors explicit commands over magic to prevent confusion.
//!
//! ## Module Structure
//!
//! - `commands`: Per-command handlers that call API and format output
//! - `render`: Output formatting (tables, colors, messages)
//! - `setup`: Argument parsing via clap, help text
//! - `styles`: Terminal styling constants
//! - `templates`: Output templates

mod commands;
mod complete;
mod render;
pub mod setup;
mod styles;
mod templates;

pub use commands::run;
