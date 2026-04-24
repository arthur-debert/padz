//! Export/import to files, and clone/migrate between stores.

use crate::commands;
use crate::error::{PadzError, Result};
use crate::model::Scope;
use crate::store::DataStore;

use super::selectors::{canonicalize_or_self, parse_selectors};
use super::PadzApi;

impl<S: DataStore> PadzApi<S> {
    pub fn export_pads<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
        nesting: commands::NestingMode,
        with_metadata: bool,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::export::run(&self.store, scope, &selectors, nesting, with_metadata)
    }

    pub fn export_pads_single_file<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
        title: &str,
        nesting: commands::NestingMode,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::export::run_single_file(&self.store, scope, &selectors, title, nesting)
    }

    pub fn export_pads_json<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
        nesting: commands::NestingMode,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::export::run_json(&self.store, scope, &selectors, nesting)
    }

    pub fn import_pads(
        &mut self,
        scope: Scope,
        paths: Vec<std::path::PathBuf>,
        import_exts: &[String],
    ) -> Result<commands::CmdResult> {
        commands::import::run(&mut self.store, scope, paths, import_exts)
    }

    /// Direction for clone/migrate: the external path is either the
    /// destination (`--to`) or the source (`--from`).
    pub fn transfer_pads_to<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        dest_path: &std::path::Path,
        mode: commands::transfer::TransferMode,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        let dest_padz = commands::transfer::resolve_target_dir(dest_path)?;
        // Refuse to transfer to the same store. For migrate the user would
        // see the pad copied over itself and then deleted — data loss.
        // Clone would no-op at best; we reject both for a clear error.
        let current_padz = self.paths.scope_dir(scope)?;
        if canonicalize_or_self(&current_padz) == canonicalize_or_self(&dest_padz) {
            return Err(PadzError::Api(format!(
                "Destination '{}' is the current store. Use a different `--to` target.",
                dest_padz.display()
            )));
        }
        let mut dest_store = commands::transfer::open_target_store(&dest_padz)?;
        commands::transfer::run(
            &mut self.store,
            scope,
            &mut dest_store,
            Scope::Project,
            &selectors,
            &dest_padz,
            mode,
        )
    }

    /// `--from <path>`: read selectors from the external store, write into
    /// the current store.
    pub fn transfer_pads_from<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        source_path: &std::path::Path,
        mode: commands::transfer::TransferMode,
    ) -> Result<commands::CmdResult> {
        let source_padz = commands::transfer::resolve_target_dir(source_path)?;
        let current_padz = self.paths.scope_dir(scope)?;
        if canonicalize_or_self(&current_padz) == canonicalize_or_self(&source_padz) {
            return Err(PadzError::Api(format!(
                "Source '{}' is the current store. Use a different `--from` target.",
                source_padz.display()
            )));
        }
        let mut source_store = commands::transfer::open_target_store(&source_padz)?;
        let selectors = parse_selectors(indexes).map_err(|e| PadzError::Api(format!("{}", e)))?;
        commands::transfer::run(
            &mut source_store,
            Scope::Project,
            &mut self.store,
            scope,
            &selectors,
            &source_padz,
            mode,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::api::test_support::make_api;
    use crate::model::Scope;

    #[test]
    fn test_api_import_pads() {
        let mut api = make_api();
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("note.md");
        std::fs::write(&file_path, "Imported Note\n\nContent").unwrap();

        let result = api
            .import_pads(Scope::Project, vec![file_path], &[".md".to_string()])
            .unwrap();

        assert!(result
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));
    }
}
