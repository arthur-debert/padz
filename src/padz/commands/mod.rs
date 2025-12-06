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
pub mod helpers;
pub mod import;
pub mod init;
pub mod list;
pub mod paths;
pub mod pinning;
pub mod purge;
pub mod search;
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
