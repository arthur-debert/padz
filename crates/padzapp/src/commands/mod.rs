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
//! - `affected_pads`: Pads that were modified
//! - `listed_pads`: Pads to display (with display indexes)
//! - `messages`: Structured messages with levels (info, success, warning, error)
//! - `pad_paths`: File paths (for `path` command)
//! - `config`: Configuration data (for `config` command)
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

use crate::config::PadzConfig;
use crate::error::{PadzError, Result};
use crate::index::DisplayPad;
use crate::model::{Pad, Scope};
use std::path::PathBuf;

pub mod config;
pub mod create;
pub mod delete;
pub mod doctor;
pub mod export;
pub mod get;
pub mod helpers;
pub mod import;
pub mod init;

pub mod paths;
pub mod pinning;
pub mod purge;
pub mod restore;

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

#[derive(Debug, Clone)]
pub enum MessageLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
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
    pub affected_pads: Vec<Pad>,
    pub listed_pads: Vec<DisplayPad>,
    pub pad_paths: Vec<PathBuf>,
    pub config: Option<PadzConfig>,
    pub messages: Vec<CmdMessage>,
}

impl CmdResult {
    pub fn add_message(&mut self, message: CmdMessage) {
        self.messages.push(message);
    }

    pub fn with_affected_pads(mut self, pads: Vec<Pad>) -> Self {
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

    pub fn with_config(mut self, config: PadzConfig) -> Self {
        self.config = Some(config);
        self
    }
}

#[derive(Debug, Clone)]
pub struct PadUpdate {
    pub index: crate::index::DisplayIndex,
    pub title: String,
    pub content: String,
}

impl PadUpdate {
    pub fn new(index: crate::index::DisplayIndex, title: String, content: String) -> Self {
        Self {
            index,
            title,
            content,
        }
    }
}
