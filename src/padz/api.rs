use crate::commands;
use crate::error::Result;
use crate::index::DisplayIndex;
use crate::model::Scope;
use crate::store::DataStore;

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

    pub fn list_pads(&self, scope: Scope, show_deleted: bool) -> Result<commands::CmdResult> {
        commands::list::run(&self.store, scope, show_deleted)
    }

    pub fn view_pads(&self, scope: Scope, indexes: &[DisplayIndex]) -> Result<commands::CmdResult> {
        commands::view::run(&self.store, scope, indexes)
    }

    pub fn delete_pads(
        &mut self,
        scope: Scope,
        indexes: &[DisplayIndex],
    ) -> Result<commands::CmdResult> {
        commands::delete::run(&mut self.store, scope, indexes)
    }

    pub fn pin_pads(
        &mut self,
        scope: Scope,
        indexes: &[DisplayIndex],
    ) -> Result<commands::CmdResult> {
        commands::pinning::pin(&mut self.store, scope, indexes)
    }

    pub fn unpin_pads(
        &mut self,
        scope: Scope,
        indexes: &[DisplayIndex],
    ) -> Result<commands::CmdResult> {
        commands::pinning::unpin(&mut self.store, scope, indexes)
    }

    pub fn update_pads(
        &mut self,
        scope: Scope,
        updates: &[commands::PadUpdate],
    ) -> Result<commands::CmdResult> {
        commands::update::run(&mut self.store, scope, updates)
    }

    pub fn pad_paths(&self, scope: Scope, indexes: &[DisplayIndex]) -> Result<commands::CmdResult> {
        commands::paths::run(&self.store, scope, indexes)
    }

    pub fn search_pads(&self, scope: Scope, term: &str) -> Result<commands::CmdResult> {
        commands::search::run(&self.store, scope, term)
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

pub use crate::commands::config::ConfigAction;
pub use commands::{CmdMessage, CmdResult, MessageLevel, PadUpdate, PadzPaths};
