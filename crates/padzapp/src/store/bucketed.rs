//! # Bucketed Store
//!
//! Wraps three [`PadStore`] instances (active, archived, deleted) to provide
//! bucket-aware storage. Each inner store manages its own subdirectory with
//! independent `data.json` and pad content files.
//!
//! Tags are stored at the scope root (shared across buckets) via a separate backend.

use super::backend::StorageBackend;
use super::pad_store::PadStore;
use super::{Bucket, DataStore, DoctorReport};
use crate::error::Result;
use crate::model::{Pad, Scope};
use crate::tags::TagEntry;
use std::path::PathBuf;
use uuid::Uuid;

/// A store that manages three lifecycle buckets (active, archived, deleted)
/// plus a scope-level tag backend.
pub struct BucketedStore<B: StorageBackend> {
    pub(crate) active: PadStore<B>,
    pub(crate) archived: PadStore<B>,
    pub(crate) deleted: PadStore<B>,
    /// Separate backend at the scope root for tags (shared across buckets).
    pub(crate) tag_backend: B,
}

impl<B: StorageBackend> BucketedStore<B> {
    pub fn new(active: B, archived: B, deleted: B, tag_backend: B) -> Self {
        Self {
            active: PadStore::with_backend(active),
            archived: PadStore::with_backend(archived),
            deleted: PadStore::with_backend(deleted),
            tag_backend,
        }
    }

    fn store(&self, bucket: Bucket) -> &PadStore<B> {
        match bucket {
            Bucket::Active => &self.active,
            Bucket::Archived => &self.archived,
            Bucket::Deleted => &self.deleted,
        }
    }

    fn store_mut(&mut self, bucket: Bucket) -> &mut PadStore<B> {
        match bucket {
            Bucket::Active => &mut self.active,
            Bucket::Archived => &mut self.archived,
            Bucket::Deleted => &mut self.deleted,
        }
    }

    /// Access the active inner store (for testing/internal use)
    pub fn active_store(&self) -> &PadStore<B> {
        &self.active
    }

    /// Access the active inner store mutably
    pub fn active_store_mut(&mut self) -> &mut PadStore<B> {
        &mut self.active
    }

    /// Synchronize all buckets with their backends.
    pub fn sync(&self, scope: crate::model::Scope) -> crate::error::Result<()> {
        self.active.sync(scope)?;
        self.archived.sync(scope)?;
        self.deleted.sync(scope)?;
        Ok(())
    }
}

impl<B: StorageBackend> DataStore for BucketedStore<B> {
    fn save_pad(&mut self, pad: &Pad, scope: Scope, bucket: Bucket) -> Result<()> {
        self.store_mut(bucket).save_pad(pad, scope)
    }

    fn get_pad(&self, id: &Uuid, scope: Scope, bucket: Bucket) -> Result<Pad> {
        self.store(bucket).get_pad(id, scope)
    }

    fn list_pads(&self, scope: Scope, bucket: Bucket) -> Result<Vec<Pad>> {
        self.store(bucket).list_pads(scope)
    }

    fn delete_pad(&mut self, id: &Uuid, scope: Scope, bucket: Bucket) -> Result<()> {
        self.store_mut(bucket).delete_pad(id, scope)
    }

    fn move_pad(&mut self, id: &Uuid, scope: Scope, from: Bucket, to: Bucket) -> Result<Pad> {
        if from == to {
            return self.get_pad(id, scope, from);
        }

        // 1. Read from source
        let pad = self.store(from).get_pad(id, scope)?;

        // 2. Write to destination (content first = orphan-safe)
        self.store_mut(to).save_pad(&pad, scope)?;

        // 3. Remove from source
        // Crash between 2 and 3: pad in both. Source doctor cleans the zombie. Safe.
        self.store_mut(from).delete_pad(id, scope)?;

        Ok(pad)
    }

    fn move_pads(
        &mut self,
        ids: &[Uuid],
        scope: Scope,
        from: Bucket,
        to: Bucket,
    ) -> Result<Vec<Pad>> {
        let mut moved = Vec::with_capacity(ids.len());
        for id in ids {
            moved.push(self.move_pad(id, scope, from, to)?);
        }
        Ok(moved)
    }

    fn get_pad_path(&self, id: &Uuid, scope: Scope, bucket: Bucket) -> Result<PathBuf> {
        self.store(bucket).get_pad_path(id, scope)
    }

    fn doctor(&mut self, scope: Scope) -> Result<DoctorReport> {
        let active_report = self.active.doctor(scope)?;
        let archived_report = self.archived.doctor(scope)?;
        let deleted_report = self.deleted.doctor(scope)?;

        Ok(DoctorReport {
            fixed_missing_files: active_report.fixed_missing_files
                + archived_report.fixed_missing_files
                + deleted_report.fixed_missing_files,
            recovered_files: active_report.recovered_files
                + archived_report.recovered_files
                + deleted_report.recovered_files,
            fixed_content_files: active_report.fixed_content_files
                + archived_report.fixed_content_files
                + deleted_report.fixed_content_files,
        })
    }

    fn load_tags(&self, scope: Scope) -> Result<Vec<TagEntry>> {
        self.tag_backend.load_tags(scope)
    }

    fn save_tags(&mut self, scope: Scope, tags: &[TagEntry]) -> Result<()> {
        self.tag_backend.save_tags(scope, tags)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scope;
    use crate::store::mem_backend::MemBackend;

    type BucketedInMemoryStore = BucketedStore<MemBackend>;

    fn make_store() -> BucketedInMemoryStore {
        BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        )
    }

    #[test]
    fn test_save_and_get_in_active() {
        let mut store = make_store();
        let pad = Pad::new("Active Pad".into(), "Content".into());
        let id = pad.metadata.id;

        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let retrieved = store.get_pad(&id, Scope::Project, Bucket::Active).unwrap();
        assert_eq!(retrieved.metadata.title, "Active Pad");
    }

    #[test]
    fn test_buckets_are_isolated() {
        let mut store = make_store();

        let active_pad = Pad::new("Active".into(), "".into());
        let deleted_pad = Pad::new("Deleted".into(), "".into());
        let archived_pad = Pad::new("Archived".into(), "".into());

        store
            .save_pad(&active_pad, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&deleted_pad, Scope::Project, Bucket::Deleted)
            .unwrap();
        store
            .save_pad(&archived_pad, Scope::Project, Bucket::Archived)
            .unwrap();

        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Active)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Deleted)
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Archived)
                .unwrap()
                .len(),
            1
        );

        assert_eq!(
            store.list_pads(Scope::Project, Bucket::Active).unwrap()[0]
                .metadata
                .title,
            "Active"
        );
    }

    #[test]
    fn test_move_pad_between_buckets() {
        let mut store = make_store();
        let pad = Pad::new("Moving Pad".into(), "Content".into());
        let id = pad.metadata.id;

        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        // Move from Active to Deleted
        let moved = store
            .move_pad(&id, Scope::Project, Bucket::Active, Bucket::Deleted)
            .unwrap();
        assert_eq!(moved.metadata.title, "Moving Pad");

        // Should be gone from Active
        assert!(store.get_pad(&id, Scope::Project, Bucket::Active).is_err());

        // Should be in Deleted
        let in_deleted = store.get_pad(&id, Scope::Project, Bucket::Deleted).unwrap();
        assert_eq!(in_deleted.metadata.title, "Moving Pad");
    }

    #[test]
    fn test_move_pads_batch() {
        let mut store = make_store();
        let pad1 = Pad::new("Pad 1".into(), "".into());
        let pad2 = Pad::new("Pad 2".into(), "".into());
        let id1 = pad1.metadata.id;
        let id2 = pad2.metadata.id;

        store
            .save_pad(&pad1, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&pad2, Scope::Project, Bucket::Active)
            .unwrap();

        let moved = store
            .move_pads(
                &[id1, id2],
                Scope::Project,
                Bucket::Active,
                Bucket::Archived,
            )
            .unwrap();
        assert_eq!(moved.len(), 2);

        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Active)
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Archived)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn test_move_same_bucket_is_noop() {
        let mut store = make_store();
        let pad = Pad::new("Same Bucket".into(), "".into());
        let id = pad.metadata.id;

        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let moved = store
            .move_pad(&id, Scope::Project, Bucket::Active, Bucket::Active)
            .unwrap();
        assert_eq!(moved.metadata.title, "Same Bucket");

        // Should still be in Active
        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Active)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn test_delete_pad_from_bucket() {
        let mut store = make_store();
        let pad = Pad::new("To Delete".into(), "".into());
        let id = pad.metadata.id;

        store
            .save_pad(&pad, Scope::Project, Bucket::Deleted)
            .unwrap();
        store
            .delete_pad(&id, Scope::Project, Bucket::Deleted)
            .unwrap();

        assert!(store.get_pad(&id, Scope::Project, Bucket::Deleted).is_err());
    }

    #[test]
    fn test_doctor_across_buckets() {
        let mut store = make_store();
        let report = store.doctor(Scope::Project).unwrap();

        // Empty store, nothing to fix
        assert_eq!(report.fixed_missing_files, 0);
        assert_eq!(report.recovered_files, 0);
        assert_eq!(report.fixed_content_files, 0);
    }

    #[test]
    fn test_tags_are_scope_level() {
        let mut store = make_store();
        let tags = vec![
            TagEntry::new("work".to_string()),
            TagEntry::new("rust".to_string()),
        ];

        store.save_tags(Scope::Project, &tags).unwrap();

        let loaded = store.load_tags(Scope::Project).unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].name, "work");
    }
}
