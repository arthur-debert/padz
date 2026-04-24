//! `FileStore`-specific API methods — format overrides for pad creation.

use crate::commands;
use crate::config::normalize_format;
use crate::error::{PadzError, Result};
use crate::index::parse_index_or_range;
use crate::model::Scope;
use crate::store::fs::FileStore;

use super::PadzApi;

impl PadzApi<FileStore> {
    /// Create a pad with an explicit format override (e.g., "md", "txt").
    /// The format is temporary — it only affects this pad's file extension.
    pub fn create_pad_with_format(
        &mut self,
        scope: Scope,
        title: String,
        content: String,
        parent: Option<&str>,
        format: &str,
    ) -> Result<commands::CmdResult> {
        let parent_selector = if let Some(p) = parent {
            Some(parse_index_or_range(p).map_err(PadzError::Api)?)
        } else {
            None
        };
        let prev_format = self.store.format_ext().to_string();
        let normalized = normalize_format(format);
        self.store.set_format(&normalized);
        let result = commands::create::run(&mut self.store, scope, title, content, parent_selector);
        self.store.set_format(&prev_format);
        result
    }
}
