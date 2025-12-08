//! # API Facade
//!
//! The API layer is a **thin facade** over the command layer. It serves as the single
//! entry point for all padz operations, regardless of the UI being used.
//!
//! ## Role and Responsibilities
//!
//! The API facade:
//! - **Dispatches** to the appropriate command function
//! - **Normalizes inputs** (e.g., converting display indexes to UUIDs)
//! - **Returns structured types** (`Result<CmdResult>`)
//!
//! ## What the API Does NOT Do
//!
//! The API explicitly avoids:
//! - **Business logic**: That belongs in `commands/*.rs`
//! - **I/O operations**: No stdout, stderr, or file formatting
//! - **Presentation concerns**: Returns data structures, not strings
//!
//! ## Generic Over DataStore
//!
//! `PadzApi<S: DataStore>` is generic over the storage backend:
//! - Production: `PadzApi<FileStore>`
//! - Testing: `PadzApi<InMemoryStore>`
//!
//! This enables testing the API layer without touching the filesystem.
//!
//! ## Testing Strategy
//!
//! API tests should verify:
//! - Correct command is called for each method
//! - Arguments are passed/transformed correctly
//! - Return types are appropriate
//!
//! API tests should **not** verify:
//! - Command logic (tested in command modules)
//! - Storage behavior (tested in store modules)

use crate::commands;
use crate::error::Result;
use crate::index::{DisplayIndex, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;
use std::str::FromStr;

/// The main API facade for padz operations.
///
/// Generic over `DataStore` to allow different storage backends.
/// All UI clients (CLI, web, etc.) should interact through this API.
pub struct PadzApi<S: DataStore> {
    store: S,
    paths: commands::PadzPaths,
}

impl<S: DataStore> PadzApi<S> {
    pub fn new(store: S, paths: commands::PadzPaths) -> Self {
        Self { store, paths }
    }

    pub fn create_pad(
        &mut self,
        scope: Scope,
        title: String,
        content: String,
    ) -> Result<commands::CmdResult> {
        commands::create::run(&mut self.store, scope, title, content)
    }

    pub fn get_pads(&self, scope: Scope, filter: PadFilter) -> Result<commands::CmdResult> {
        commands::get::run(&self.store, scope, filter)
    }

    pub fn view_pads<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::view::run(&self.store, scope, &selectors)
    }

    pub fn delete_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::delete::run(&mut self.store, scope, &selectors)
    }

    pub fn pin_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::pinning::pin(&mut self.store, scope, &selectors)
    }

    pub fn unpin_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::pinning::unpin(&mut self.store, scope, &selectors)
    }

    pub fn update_pads(
        &mut self,
        scope: Scope,
        updates: &[commands::PadUpdate],
    ) -> Result<commands::CmdResult> {
        commands::update::run(&mut self.store, scope, updates)
    }

    pub fn purge_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        skip_confirm: bool,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::purge::run(&mut self.store, scope, &selectors, skip_confirm)
    }

    pub fn export_pads<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::export::run(&self.store, scope, &selectors)
    }

    pub fn import_pads(
        &mut self,
        scope: Scope,
        paths: Vec<std::path::PathBuf>,
        import_exts: &[String],
    ) -> Result<commands::CmdResult> {
        commands::import::run(&mut self.store, scope, paths, import_exts)
    }

    pub fn doctor(&mut self, scope: Scope) -> Result<commands::CmdResult> {
        commands::doctor::run(&mut self.store, scope)
    }

    pub fn pad_paths<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::paths::run(&self.store, scope, &selectors)
    }

    pub fn get_path_by_id(&self, scope: Scope, id: uuid::Uuid) -> Result<std::path::PathBuf> {
        self.store.get_pad_path(&id, scope)
    }

    pub fn config(&self, scope: Scope, action: ConfigAction) -> Result<commands::CmdResult> {
        commands::config::run(&self.paths, scope, action)
    }

    pub fn init(&self, scope: Scope) -> Result<commands::CmdResult> {
        commands::init::run(&self.paths, scope)
    }

    pub fn paths(&self) -> &commands::PadzPaths {
        &self.paths
    }
}

fn parse_selectors<I: AsRef<str>>(inputs: &[I]) -> Result<Vec<PadSelector>> {
    // 1. Try to parse ALL inputs as DisplayIndex
    let all_indexes: std::result::Result<Vec<DisplayIndex>, _> = inputs
        .iter()
        .map(|s| DisplayIndex::from_str(s.as_ref()))
        .collect();

    if let Ok(indexes) = all_indexes {
        return Ok(indexes.into_iter().map(PadSelector::Index).collect());
    }

    // 2. If any failed (meaning there are non-index strings), treat as ONE search query
    // Join all parts with space
    let search_term = inputs
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<&str>>()
        .join(" ");

    Ok(vec![PadSelector::Title(search_term)])
}

pub use crate::commands::config::ConfigAction;
pub use commands::get::{PadFilter, PadStatusFilter};
pub use commands::{CmdMessage, CmdResult, MessageLevel, PadUpdate, PadzPaths};
