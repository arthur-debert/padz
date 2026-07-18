//! # Command Layer
//!
//! This module contains the **core business logic** of padz. Each command lives in its
//! own submodule and implements pure Rust functions that operate on data types.
//!
//! ## Role and Responsibilities
//!
//! Commands are where the real work happens:
//! - Implement the actual logic for each operation
//! - Operate on `Pad`, `Metadata`, and other domain types
//! - Return structured `CmdResult` values or operation-specific semantic outcomes
//! - Are completely UI-agnostic
//!
//! ## What Commands Do NOT Do
//!
//! Commands explicitly avoid:
//! - **Any I/O**: No stdout, stderr, file formatting, or terminal concerns
//! - **Argument parsing**: That's the CLI layer's job
//! - **Exit codes**: Return `Result`, let the caller decide
//! - **User interaction**: No prompts, confirmations (return data, UI decides)
//!
//! ## Structured Returns
//!
//! Most commands return [`CmdResult`], not strings. This struct carries:
//! - `affected_pads`: Pads that were modified (as `DisplayPad` with post-operation index)
//! - `listed_pads`: Pads to display (as `DisplayPad` with current index)
//! - `messages`: Structured messages with levels (info, success, warning, error)
//! - `pad_paths`: File paths (for `path` command)
//!
//! Both `affected_pads` and `listed_pads` use [`DisplayPad`], which pairs a [`Pad`]
//! with its canonical [`DisplayIndex`]. This ensures consistent representation
//! throughout the API—clients always receive pads with their resolved indexes.
//!
//! Initialization, doctor, purge, import, tag catalog/mutation, and
//! artifact-producing commands use dedicated outcome types where a generic
//! result would obscure the operation's facts. Pad mutations that still use [`CmdResult`] attach
//! presentation-free [`CmdOutcome`] and [`CmdNotice`] facts. The UI layer (CLI,
//! web, etc.) decides how to render every result.
//!
//! ## Testing Strategy
//!
//! **This is where the lion's share of testing lives.**
//!
//! Command tests should:
//! - Use `InMemoryStore` to avoid filesystem dependencies
//! - Test all logic branches and edge cases
//! - Verify correct `CmdResult` contents
//! - Test error conditions
//!
//! ## Command Modules
//!
//! - [`create`]: Create new pads
//! - [`list`]: List pads in a scope
//! - [`view`]: Retrieve pad content
//! - [`update`]: Modify existing pads
//! - [`delete`]: Soft-delete pads
//! - [`pinning`]: Pin/unpin pads
//! - [`purge`]: Permanently remove deleted pads
//! - [`search`]: Full-text search
//! - [`export`]: Export pads to archive
//! - [`import`]: Import pads from files
//! - [`paths`]: Get filesystem paths to pads
//! - [`init`]: Initialize scope directories
//! - [`doctor`]: Verify and fix data consistency
//! - [`tags`]: List and mutate the tag registry
//! - [`tagging`]: Assign and remove tags on selected pads
//! - [`helpers`]: Shared utilities (index resolution, etc.)

use crate::error::{PadzError, Result};
use crate::index::DisplayPad;
use crate::model::Scope;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Controls how nested (parent/child) pads are rendered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NestingMode {
    /// Show only the selected pad(s), no children (legacy behavior).
    Flat,
    /// Recursively include children, no indentation.
    #[default]
    Tree,
    /// Recursively include children, with 4-space indentation per nesting level.
    Indented,
}

/// A semantic, presentation-free fact a command wants the caller to surface.
///
/// Unlike [`CmdMessage`], notices carry no authored prose. CLI clients can render
/// them for people while structured clients can branch on `kind` and fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CmdNotice {
    /// A pin/unpin request found the pad in the requested state already.
    AlreadyPinned {
        path: Vec<crate::index::DisplayIndex>,
    },
    AlreadyUnpinned {
        path: Vec<crate::index::DisplayIndex>,
    },
    /// A move request found the pad under the requested parent already.
    AlreadyAtDestination {
        path: Vec<crate::index::DisplayIndex>,
    },
    /// A status request found the pad in the requested state already.
    AlreadyInStatus {
        path: Vec<crate::index::DisplayIndex>,
        status: crate::model::TodoStatus,
    },
    /// A completed-pad deletion request found no completed pads.
    NoCompletedPads,
}

/// How a pad's content reached the update command.
///
/// The distinction preserves the compatible human result while structured
/// clients can identify the operation without parsing its sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateKind {
    /// A typed [`PadUpdate`] changed explicit pad fields.
    Structured,
    /// Raw content was parsed and applied to one or more pads.
    Content,
    /// The store refreshed a pad after its backing file changed externally.
    Refresh,
}

/// A semantic, presentation-free fact about a completed pad mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CmdOutcome {
    /// A pad's title/content/status fields were updated.
    Updated {
        path: Vec<crate::index::DisplayIndex>,
        title: String,
        update_kind: UpdateKind,
    },
    /// A pad changed to the requested todo status.
    StatusChanged {
        path: Vec<crate::index::DisplayIndex>,
        status: crate::model::TodoStatus,
    },
}

pub mod archive;
pub mod create;
pub mod delete;
pub mod doctor;
pub mod get;
pub mod helpers;
pub mod init;
pub mod io;
pub mod move_pads;

// Preserve pre-split paths: `commands::export`, `commands::import`.
pub use io::{export, import};

pub mod inline_metadata;
pub mod metadata_apply;
pub mod metadata_schema;
pub mod paths;
pub mod pinning;
pub mod purge;
pub mod restore;
pub mod status;
pub mod tagging;
pub mod tags;
pub mod transfer;

pub mod unarchive;
pub mod update;
pub mod uuid;
pub mod view;

/// The filesystem locations a `PadzApi` operates against, supplied by the
/// caller rather than discovered here.
#[derive(Debug, Clone)]
pub struct PadzPaths {
    pub project: Option<PathBuf>,
    pub global: PathBuf,
    /// The user's home directory, used only as the stopping point when walking
    /// upward to resolve a user-supplied transfer path. `None` means "walk to
    /// the filesystem root". See `init::PadzEnv::home_dir`.
    pub home: Option<PathBuf>,
}

impl PadzPaths {
    pub fn scope_dir(&self, scope: Scope) -> Result<PathBuf> {
        match scope {
            Scope::Project => self
                .project
                .clone()
                .ok_or_else(|| PadzError::Store("Project scope is not available".to_string())),
            Scope::Global => Ok(self.global.clone()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CmdMessage {
    pub level: MessageLevel,
    pub content: String,
}

impl CmdMessage {
    pub fn info(content: impl Into<String>) -> Self {
        Self {
            level: MessageLevel::Info,
            content: content.into(),
        }
    }

    pub fn success(content: impl Into<String>) -> Self {
        Self {
            level: MessageLevel::Success,
            content: content.into(),
        }
    }

    pub fn warning(content: impl Into<String>) -> Self {
        Self {
            level: MessageLevel::Warning,
            content: content.into(),
        }
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self {
            level: MessageLevel::Error,
            content: content.into(),
        }
    }
}

#[derive(Debug, Default)]
pub struct CmdResult {
    pub affected_pads: Vec<DisplayPad>,
    pub listed_pads: Vec<DisplayPad>,
    /// Nesting depths for listed_pads (parallel to listed_pads).
    /// Empty means all pads are at depth 0.
    pub listed_depths: Vec<usize>,
    pub pad_paths: Vec<PathBuf>,
    pub messages: Vec<CmdMessage>,
    /// Semantic notices that clients render or inspect without parsing English.
    pub notices: Vec<CmdNotice>,
    /// Semantic successful outcomes that clients inspect without parsing English.
    pub outcomes: Vec<CmdOutcome>,
    /// The nesting mode used to produce listed_pads.
    pub nesting: NestingMode,
}

impl CmdResult {
    pub fn add_message(&mut self, message: CmdMessage) {
        self.messages.push(message);
    }

    pub fn with_affected_pads(mut self, pads: Vec<DisplayPad>) -> Self {
        self.affected_pads = pads;
        self
    }

    pub fn with_listed_pads(mut self, pads: Vec<DisplayPad>) -> Self {
        self.listed_pads = pads;
        self
    }

    pub fn with_pad_paths(mut self, paths: Vec<PathBuf>) -> Self {
        self.pad_paths = paths;
        self
    }
}

/// Result from commands that modify pads.
///
/// Provides structured output for unified CLI rendering:
/// - `affected_pads`: The pads that were modified (rendered as a list)
/// - `trailing_messages`: Info/warning messages shown after the pad list
///
/// The CLI layer generates the start message (e.g., "Completing 2 pads...")
/// based on the action and affected_pads count.
#[derive(Debug, Default)]
pub struct ModificationResult {
    /// Pads that were modified by the operation
    pub affected_pads: Vec<DisplayPad>,
    /// Messages shown after the pad list (info, warnings, errors)
    pub trailing_messages: Vec<CmdMessage>,
}

impl ModificationResult {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pads(mut self, pads: Vec<DisplayPad>) -> Self {
        self.affected_pads = pads;
        self
    }

    pub fn add_info(&mut self, content: impl Into<String>) {
        self.trailing_messages.push(CmdMessage::info(content));
    }

    /// Convert to CmdResult for backward compatibility
    pub fn into_cmd_result(self) -> CmdResult {
        CmdResult {
            affected_pads: self.affected_pads,
            messages: self.trailing_messages,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone)]
pub struct PadUpdate {
    pub index: crate::index::DisplayIndex,
    pub title: String,
    pub content: String,
    pub status: Option<crate::model::TodoStatus>,
    pub path: Option<Vec<crate::index::DisplayIndex>>,
}

impl PadUpdate {
    pub fn new(index: crate::index::DisplayIndex, title: String, content: String) -> Self {
        Self {
            index,
            title,
            content,
            status: None,
            path: None,
        }
    }

    pub fn with_status(mut self, status: crate::model::TodoStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_path(mut self, path: Vec<crate::index::DisplayIndex>) -> Self {
        self.path = Some(path);
        self
    }
}
