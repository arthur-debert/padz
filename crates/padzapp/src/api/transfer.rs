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
    ) -> Result<commands::export::ExportOutcome> {
        let selectors = parse_selectors(indexes)?;
        commands::export::run(&self.store, scope, &selectors, nesting, with_metadata)
    }

    pub fn export_pads_single_file<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
        title: &str,
        nesting: commands::NestingMode,
    ) -> Result<commands::export::ExportOutcome> {
        let selectors = parse_selectors(indexes)?;
        commands::export::run_single_file(&self.store, scope, &selectors, title, nesting)
    }

    pub fn export_pads_json<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
        nesting: commands::NestingMode,
    ) -> Result<commands::export::ExportOutcome> {
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
        let dest_padz =
            commands::transfer::resolve_target_dir(dest_path, self.paths.home.as_deref())?;
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
        let source_padz =
            commands::transfer::resolve_target_dir(source_path, self.paths.home.as_deref())?;
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
    use super::*;
    use crate::api::test_support::{make_api, make_store, TestStore};
    use crate::api::{PadzApi, PadzPaths};
    use crate::commands::transfer::TransferMode;
    use crate::model::Scope;
    use crate::store::Bucket;
    use std::path::PathBuf;

    /// Initialize a `.padz/<bucket>` layout at `dir` so it looks like an
    /// initialized store on disk — what `open_target_store` requires.
    fn init_layout(dir: &std::path::Path) {
        std::fs::create_dir_all(dir.join("active")).unwrap();
        std::fs::create_dir_all(dir.join("archived")).unwrap();
        std::fs::create_dir_all(dir.join("deleted")).unwrap();
    }

    /// Build an API with `paths.project` pointing at the supplied dir.
    /// The store remains in-memory; the path is only consulted for the
    /// same-store-rejection check.
    fn make_api_at(project_dir: PathBuf) -> PadzApi<TestStore> {
        PadzApi::new(
            make_store(),
            PadzPaths {
                project: Some(project_dir),
                global: PathBuf::from("/tmp/global"),
                home: None,
            },
        )
    }

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

    // ------------------------------------------------------------------------
    // transfer_pads_to
    // ------------------------------------------------------------------------

    #[test]
    fn test_transfer_pads_to_clones_into_destination() {
        let mut api = make_api();
        // Source: create two pads in-memory.
        api.create_pad(Scope::Project, "A".into(), "alpha".into(), None)
            .unwrap();
        api.create_pad(Scope::Project, "B".into(), "beta".into(), None)
            .unwrap();

        // Destination on disk.
        let temp = tempfile::tempdir().unwrap();
        let dest_padz = temp.path().join(".padz");
        init_layout(&dest_padz);

        let empty: &[&str] = &[];
        let result = api
            .transfer_pads_to(Scope::Project, empty, &dest_padz, TransferMode::Clone)
            .unwrap();

        assert!(
            result
                .messages
                .iter()
                .any(|m| m.content.contains("Cloned 2 pad(s)")),
            "expected Cloned message; got {:?}",
            result.messages
        );

        // Source must still hold both pads after a clone.
        let still_there = api
            .get_pads(Scope::Project, Default::default(), &[] as &[String])
            .unwrap();
        assert_eq!(still_there.listed_pads.len(), 2);
    }

    #[test]
    fn test_transfer_pads_to_rejects_same_store() {
        // The api's "project" path equals the destination path → reject.
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().to_path_buf();
        let project_padz = project.join(".padz");
        init_layout(&project_padz);

        let mut api = make_api_at(project_padz.clone());
        api.create_pad(Scope::Project, "A".into(), "".into(), None)
            .unwrap();

        let empty: &[&str] = &[];
        let err = api
            .transfer_pads_to(Scope::Project, empty, &project_padz, TransferMode::Clone)
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("current store") && msg.contains("--to"),
            "expected same-store rejection mentioning --to; got: {msg}"
        );
    }

    #[test]
    fn test_transfer_pads_to_missing_dest_errors() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "A".into(), "".into(), None)
            .unwrap();
        let temp = tempfile::tempdir().unwrap();
        let missing = temp.path().join("nope");
        let empty: &[&str] = &[];
        let err = api
            .transfer_pads_to(Scope::Project, empty, &missing, TransferMode::Clone)
            .unwrap_err();
        // resolve_target_dir fails to canonicalize a missing path.
        assert!(err.to_string().to_lowercase().contains("target"));
    }

    // ------------------------------------------------------------------------
    // transfer_pads_from
    // ------------------------------------------------------------------------

    #[test]
    fn test_transfer_pads_from_pulls_into_current_store() {
        // Source: a real on-disk store with one pad.
        let temp = tempfile::tempdir().unwrap();
        let src_padz = temp.path().join(".padz");
        init_layout(&src_padz);
        let mut src_store = commands::transfer::open_target_store(&src_padz).unwrap();
        commands::create::run(
            &mut src_store,
            Scope::Project,
            "FromDisk".into(),
            "content".into(),
            None,
        )
        .unwrap();

        // API points at a different in-memory store.
        let mut api = make_api();
        let empty: &[&str] = &[];
        let result = api
            .transfer_pads_from(Scope::Project, empty, &src_padz, TransferMode::Clone)
            .unwrap();

        assert!(
            result
                .messages
                .iter()
                .any(|m| m.content.contains("Cloned 1 pad(s)")),
            "expected Cloned message; got {:?}",
            result.messages
        );

        let landed = api
            .get_pads(Scope::Project, Default::default(), &[] as &[String])
            .unwrap();
        assert_eq!(landed.listed_pads.len(), 1);
        assert_eq!(landed.listed_pads[0].pad.metadata.title, "FromDisk");
    }

    #[test]
    fn test_transfer_pads_from_migrate_empties_source_on_disk() {
        // The on-disk source loses its pad after a migrate pulls it.
        let temp = tempfile::tempdir().unwrap();
        let src_padz = temp.path().join(".padz");
        init_layout(&src_padz);
        {
            let mut src_store = commands::transfer::open_target_store(&src_padz).unwrap();
            commands::create::run(
                &mut src_store,
                Scope::Project,
                "Moveme".into(),
                "".into(),
                None,
            )
            .unwrap();
        }

        let mut api = make_api();
        let empty: &[&str] = &[];
        api.transfer_pads_from(Scope::Project, empty, &src_padz, TransferMode::Migrate)
            .unwrap();

        // Re-open and assert empty.
        let src_store = commands::transfer::open_target_store(&src_padz).unwrap();
        let remaining = src_store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert!(
            remaining.is_empty(),
            "migrate from a disk store should leave it empty; got {:?}",
            remaining
        );
    }

    #[test]
    fn test_transfer_pads_from_rejects_same_store() {
        let temp = tempfile::tempdir().unwrap();
        let project = temp.path().to_path_buf();
        let project_padz = project.join(".padz");
        init_layout(&project_padz);

        let mut api = make_api_at(project_padz.clone());
        let empty: &[&str] = &[];
        let err = api
            .transfer_pads_from(Scope::Project, empty, &project_padz, TransferMode::Clone)
            .unwrap_err();
        let msg = err.to_string();
        assert!(
            msg.contains("current store") && msg.contains("--from"),
            "expected same-store rejection mentioning --from; got: {msg}"
        );
    }
}
