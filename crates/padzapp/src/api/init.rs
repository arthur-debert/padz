//! Store initialization and linking.

use crate::commands;
use crate::error::Result;
use crate::model::Scope;
use crate::store::DataStore;

use super::PadzApi;

impl<S: DataStore> PadzApi<S> {
    /// Initializes `scope` and returns its scope and resolved store path as data.
    pub fn init(&self, scope: Scope) -> Result<commands::init::InitializationOutcome> {
        commands::init::run(&self.paths, scope)
    }

    /// Creates a link and returns the canonical initialized target as data.
    pub fn init_link(
        &self,
        local_padz: &std::path::Path,
        target: &std::path::Path,
    ) -> Result<commands::init::InitializationOutcome> {
        commands::init::link(local_padz, target)
    }

    /// Removes the local link and returns a typed unlink action.
    pub fn init_unlink(
        &self,
        local_padz: &std::path::Path,
    ) -> Result<commands::init::InitializationOutcome> {
        commands::init::unlink(local_padz)
    }
}
