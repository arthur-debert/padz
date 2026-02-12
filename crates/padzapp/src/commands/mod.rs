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
//! - Return structured `CmdResult` with affected pads and messages
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
//! Commands return [`CmdResult`], not strings. This struct carries:
//! - `affected_pads`: Pads that were modified (as `DisplayPad` with post-operation index)
//! - `listed_pads`: Pads to display (as `DisplayPad` with current index)
//! - `messages`: Structured messages with levels (info, success, warning, error)
//! - `pad_paths`: File paths (for `path` command)
//! - `config`: Configuration data (for `config` command)
//!
//! Both `affected_pads` and `listed_pads` use [`DisplayPad`], which pairs a [`Pad`]
//! with its canonical [`DisplayIndex`]. This ensures consistent representation
//! throughout the API—clients always receive pads with their resolved indexes.
//!
//! The UI layer (CLI, web, etc.) then decides how to render this data.
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
//! - [`config`]: Manage configuration
//! - [`init`]: Initialize scope directories
//! - [`doctor`]: Verify and fix data consistency
//! - [`helpers`]: Shared utilities (index resolution, etc.)

use crate::error::{PadzError, Result};
use crate::index::DisplayPad;
use crate::model::Scope;
use serde::Serialize;
use std::path::PathBuf;

pub mod create;
pub mod delete;
pub mod doctor;
pub mod export;
pub mod get;
pub mod helpers;
pub mod import;
pub mod init;
pub mod move_pads;

pub mod paths;
pub mod pinning;
pub mod purge;
pub mod restore;
pub mod status;
pub mod tagging;
pub mod tags;

pub mod update;
pub mod view;

#[derive(Debug, Clone)]
pub struct PadzPaths {
    pub project: Option<PathBuf>,
    pub global: PathBuf,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
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
    pub pad_paths: Vec<PathBuf>,
    pub messages: Vec<CmdMessage>,
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
