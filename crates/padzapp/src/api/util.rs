//! Low-level / ancillary API methods: path queries, pad refresh/remove, doctor.

use crate::commands;
use crate::error::Result;
use crate::model::{Pad, Scope};
use crate::store::DataStore;

use super::selectors::parse_selectors;
use super::PadzApi;

impl<S: DataStore> PadzApi<S> {
    /// Reconciles the store and returns a clean/repaired outcome with direct counts.
    pub fn doctor(&mut self, scope: Scope) -> Result<commands::doctor::DoctorOutcome> {
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

    pub fn pad_uuids<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::uuid::run(&self.store, scope, &selectors)
    }

    pub fn get_path_by_id(&self, scope: Scope, id: uuid::Uuid) -> Result<std::path::PathBuf> {
        use crate::store::Bucket;
        self.store.get_pad_path(&id, scope, Bucket::Active)
    }

    /// Re-reads a pad from disk and syncs metadata (title, updated_at).
    /// Returns None if the file content is empty (pad is hard-deleted).
    pub fn refresh_pad(&mut self, scope: Scope, id: &uuid::Uuid) -> Result<Option<Pad>> {
        use crate::store::Bucket;
        let pad = self.store.get_pad(id, scope, Bucket::Active)?;
        if pad.content.trim().is_empty() {
            self.store.delete_pad(id, scope, Bucket::Active)?;
            return Ok(None);
        }
        let mut updated = pad;
        let content = updated.content.clone();
        updated.update_from_raw(&content);
        self.store.save_pad(&updated, scope, Bucket::Active)?;
        Ok(Some(updated))
    }

    /// Hard-deletes a pad (file + metadata). Used for cleanup of aborted creates.
    pub fn remove_pad(&mut self, scope: Scope, id: uuid::Uuid) -> Result<()> {
        use crate::store::Bucket;
        self.store.delete_pad(&id, scope, Bucket::Active)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::test_support::make_api;
    use crate::api::PadFilter;
    use crate::model::Scope;
    use crate::store::backend::StorageBackend;
    use std::path::PathBuf;

    #[test]
    fn test_api_doctor() {
        let mut api = make_api();

        let result = api.doctor(Scope::Project).unwrap();

        assert_eq!(
            result,
            crate::commands::doctor::DoctorOutcome::Clean {
                missing_files: 0,
                recovered_files: 0
            }
        );
    }

    #[test]
    fn test_api_paths_accessor() {
        let api = make_api();

        let paths = api.paths();

        assert_eq!(paths.project, Some(PathBuf::from("/tmp/test")));
        assert_eq!(paths.global, PathBuf::from("/tmp/global"));
    }

    #[test]
    fn test_api_refresh_pad_updates_title() {
        let mut api = make_api();
        let result = api
            .create_pad(Scope::Project, "Original Title".into(), "Body".into(), None)
            .unwrap();
        let pad_id = result.affected_pads[0].pad.metadata.id;

        api.store
            .active
            .backend
            .write_content(&pad_id, Scope::Project, "New Title\n\nNew body")
            .unwrap();

        let refreshed = api.refresh_pad(Scope::Project, &pad_id).unwrap();
        assert!(refreshed.is_some());
        let pad = refreshed.unwrap();
        assert_eq!(pad.metadata.title, "New Title");
        assert_eq!(pad.content, "New Title\n\nNew body");
    }

    #[test]
    fn test_api_refresh_pad_empty_deletes() {
        let mut api = make_api();
        let result = api
            .create_pad(Scope::Project, "Will Be Empty".into(), "".into(), None)
            .unwrap();
        let pad_id = result.affected_pads[0].pad.metadata.id;

        api.store
            .active
            .backend
            .write_content(&pad_id, Scope::Project, "   \n\n   ")
            .unwrap();

        let refreshed = api.refresh_pad(Scope::Project, &pad_id).unwrap();
        assert!(refreshed.is_none());

        let list = api
            .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
            .unwrap();
        assert_eq!(list.listed_pads.len(), 0);
    }

    #[test]
    fn test_api_remove_pad() {
        let mut api = make_api();
        let result = api
            .create_pad(Scope::Project, "To Remove".into(), "Content".into(), None)
            .unwrap();
        let pad_id = result.affected_pads[0].pad.metadata.id;

        api.remove_pad(Scope::Project, pad_id).unwrap();

        let list = api
            .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
            .unwrap();
        assert_eq!(list.listed_pads.len(), 0);
    }
}
