//! Store initialization and linking.

use crate::commands;
use crate::error::Result;
use crate::model::Scope;
use crate::store::DataStore;

use super::PadzApi;

impl<S: DataStore> PadzApi<S> {
    pub fn init(&self, scope: Scope) -> Result<commands::CmdResult> {
        commands::init::run(&self.paths, scope)
    }

    pub fn init_link(
        &self,
        local_padz: &std::path::Path,
        target: &std::path::Path,
    ) -> Result<commands::CmdResult> {
        commands::init::link(local_padz, target)
    }

    pub fn init_unlink(&self, local_padz: &std::path::Path) -> Result<commands::CmdResult> {
        commands::init::unlink(local_padz)
    }
}
