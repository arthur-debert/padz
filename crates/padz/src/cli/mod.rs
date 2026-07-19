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
//! Behavior depends on whether stdin has piped content:
//!
//! - **With pipe**: `cat file.txt | padz` expands to `padz create` with piped content
//! - **Without pipe**: `padz` expands to `padz list`
//!
//! The "Read" operation is 90% of usage—it should be the path of least resistance.
//! Piped content takes precedence, enabling quick note capture from shell pipelines.
//!
//! ### Smart Create (`padz create`)
//!
//! Priority order for content source — declared as an input chain in
//! [`input`], resolved before dispatch, and handed to the handler as one typed
//! [`input::RequestContent`]:
//!
//! 1. **Title Argument, used directly** (highest priority)
//!    - `padz create --no-editor "Meeting Notes"`, or todos mode with a title.
//!    - Uses the args verbatim. **Skips editor, and does not read stdin** —
//!      even if something is piped.
//!
//! 2. **Piped Input**
//!    - `echo "foo" | padz create`
//!    - Creates pad with piped content. **Skips editor**. A piped-but-empty
//!      stdin aborts the create; it does not fall through to the editor.
//!
//! 3. **Editor** (when nothing above applies)
//!    - `padz create "Meeting Notes"` on a terminal.
//!    - Uses the argument as title and opens the editor for the body.
//!
//! After saving from editor, pad content is automatically copied to clipboard.
//! Use `--no-editor` flag to skip opening the editor.
//!
//! **The clipboard is not an input source.** Padz writes pad text *to* the
//! clipboard after a pad is saved or viewed; it never reads the clipboard to
//! pre-fill one. Earlier revisions of this doc described a "clipboard fallback"
//! that pre-filled the editor — that path was never wired, and the claim is
//! recorded here only to keep it from being reintroduced as new behavior.
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
//! - `commands`: App construction, state wiring, and dispatch
//! - `input`: Declarative request-input precedence for create/edit
//! - `handlers`: Thin typed adapters — extract args, call the API, return a typed result
//! - `result`: The typed, mode-independent result each handler returns
//! - `render`: Render-time view derivation for standout's templates
//! - `setup`: Argument parsing via clap, help text, and naked-invocation resolution

pub mod clipboard;
pub mod commands;
mod complete;
pub mod editor;
pub mod env;
pub mod errors;
pub mod handlers;
pub mod input;
pub mod render;
pub mod result;
pub mod setup;
pub mod views;

pub use commands::run;
