use super::backend::StorageBackend;
use super::{DataStore, DoctorReport};
use crate::error::{PadzError, Result};
use crate::model::{Metadata, Pad, Scope};
use std::path::PathBuf;
use uuid::Uuid;

use std::time::SystemTime;

pub struct PadStore<B: StorageBackend> {
    /// The underlying storage backend.
    /// Exposed as pub(crate) for testing and internal access only.
    pub(crate) backend: B,
}

impl<B: StorageBackend> PadStore<B> {
    pub fn with_backend(backend: B) -> Self {
        Self { backend }
    }

    /// Explicitly synchronize the store with the backend.
    /// This is automatically called by list_pads, but can be called manually.
    pub fn sync(&self, scope: Scope) -> Result<()> {
        self.reconcile(scope)?;
        Ok(())
    }

    /// Internal reconciliation logic used by both sync and doctor
    /// Takes &self because StorageBackend handles internal mutability (or is stateless i/o)
    fn reconcile(&self, scope: Scope) -> Result<(DoctorReport, bool)> {
        if !self.backend.scope_available(scope) {
            return Ok((DoctorReport::default(), false));
        }

        let mut meta_map = self.backend.load_index(scope)?;
        let mut report = DoctorReport::default();
        let mut changes = false;

        // 1. Identify valid files and sync their state
        let found_ids = self.backend.list_content_ids(scope)?;

        for id in &found_ids {
            let mtime = self
                .backend
                .content_mtime(id, scope)?
                .unwrap_or_else(|| SystemTime::now().into());

            // Read content if:
            // a) Orphan (not in DB)
            // b) File is newer than DB entry
            let needs_read = match meta_map.get(id) {
                None => true,
                Some(meta) => mtime > meta.updated_at,
            };

            if needs_read {
                // Best effort read
                let content_raw = self.backend.read_content(id, scope)?.unwrap_or_default();

                // Check for empty/useless files
                if content_raw.trim().is_empty() {
                    // Delete empty file
                    self.backend.delete_content(id, scope)?;
                    if meta_map.remove(id).is_some() {
                        changes = true;
                    }
                    continue;
                }

                // Update/Add to DB
                if let Some((title, normalized_content)) =
                    crate::model::parse_pad_content(&content_raw)
                {
                    if let Some(meta) = meta_map.get_mut(id) {
                        // Update existing
                        if meta.title != title || meta.updated_at != mtime {
                            meta.title = title;
                            meta.updated_at = mtime;
                            changes = true;
                        }
                    } else {
                        // New / Orphan
                        let created = mtime;

                        let new_meta = Metadata {
                            id: *id,
                            created_at: created,
                            updated_at: mtime,
                            is_pinned: false,
                            pinned_at: None,
                            is_deleted: false,
                            deleted_at: None,
                            delete_protected: false,
                            parent_id: None,
                            title,
                        };
                        meta_map.insert(*id, new_meta);
                        report.recovered_files += 1;
                        changes = true;

                        // Recovery normalization (optional)
                        if content_raw != normalized_content
                            && self
                                .backend
                                .write_content(id, scope, &normalized_content)
                                .is_ok()
                        {
                            report.fixed_content_files += 1;
                        }
                    }
                }
            }
        }

        // 2. Remove DB entries that have no files (Zombies)
        let db_ids: Vec<Uuid> = meta_map.keys().cloned().collect();
        for id in db_ids {
            if !found_ids.contains(&id) {
                meta_map.remove(&id);
                report.fixed_missing_files += 1;
                changes = true;
            }
        }

        if changes {
            self.backend.save_index(scope, &meta_map)?;
        }

        Ok((report, changes))
    }
}

impl<B: StorageBackend> DataStore for PadStore<B> {
    fn save_pad(&mut self, pad: &Pad, scope: Scope) -> Result<()> {
        // 1. Write content FIRST (Atomic) to avoid Zombies
        self.backend
            .write_content(&pad.metadata.id, scope, &pad.content)?;

        // 2. Update Index
        let mut index = self.backend.load_index(scope)?;
        index.insert(pad.metadata.id, pad.metadata.clone());
        self.backend.save_index(scope, &index)?;

        Ok(())
    }

    fn get_pad(&self, id: &Uuid, scope: Scope) -> Result<Pad> {
        let index = self.backend.load_index(scope)?;
        let metadata = index.get(id).ok_or(PadzError::PadNotFound(*id))?.clone();

        // If content is missing, return empty string (self-healing on next sync)
        let content = self.backend.read_content(id, scope)?.unwrap_or_default();

        Ok(Pad { metadata, content })
    }

    fn list_pads(&self, scope: Scope) -> Result<Vec<Pad>> {
        // Sync first!
        let _ = self.reconcile(scope);

        // Then list from "clean" state
        let index = self.backend.load_index(scope)?;
        let mut pads = Vec::new();

        for (id, metadata) in index {
            let content = self.backend.read_content(&id, scope)?.unwrap_or_default();
            pads.push(Pad { metadata, content });
        }

        Ok(pads)
    }

    fn delete_pad(&mut self, id: &Uuid, scope: Scope) -> Result<()> {
        // 1. Update Index FIRST? Or Content FIRST?
        // If we delete content first, and index update fails -> Zombie (fixed by sync).
        // If we delete index first, and content delete fails -> Orphan (fixed by sync).
        // Bias towards Orphan is safer (data not lost if it was a mistake).
        // But for explicit delete, we want it gone.
        // Current FileStore: remove from metadata first.

        let mut index = self.backend.load_index(scope)?;
        if index.remove(id).is_none() {
            return Err(PadzError::PadNotFound(*id));
        }
        self.backend.save_index(scope, &index)?;

        // 2. Delete Content
        self.backend.delete_content(id, scope)?;

        Ok(())
    }

    fn get_pad_path(&self, id: &Uuid, scope: Scope) -> Result<PathBuf> {
        self.backend.content_path(id, scope)
    }

    fn doctor(&mut self, scope: Scope) -> Result<DoctorReport> {
        let (report, _) = self.reconcile(scope)?;
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::mem_backend::MemBackend;
    use chrono::{Duration, Utc};

    fn make_store() -> PadStore<MemBackend> {
        PadStore::with_backend(MemBackend::new())
    }

    // --- Orphan Recovery Tests ---

    #[test]
    fn test_doctor_recovers_orphan_content() {
        let backend = MemBackend::new();
        let orphan_id = Uuid::new_v4();

        // Create orphan: content exists but no index entry
        backend
            .write_content(&orphan_id, Scope::Project, "Orphan Title\n\nOrphan body")
            .unwrap();

        let mut store = PadStore::with_backend(backend);
        let report = store.doctor(Scope::Project).unwrap();

        assert_eq!(report.recovered_files, 1);
        assert_eq!(report.fixed_missing_files, 0);

        // Verify it's now in the store
        let pad = store.get_pad(&orphan_id, Scope::Project).unwrap();
        assert_eq!(pad.metadata.title, "Orphan Title");
        assert_eq!(pad.metadata.id, orphan_id);
    }

    #[test]
    fn test_doctor_normalizes_orphan_content() {
        let backend = MemBackend::new();
        let orphan_id = Uuid::new_v4();

        // Create orphan with non-normalized content (extra blank lines)
        backend
            .write_content(&orphan_id, Scope::Project, "\n\nTitle\n\n\n\nBody\n\n")
            .unwrap();

        let mut store = PadStore::with_backend(backend);
        let report = store.doctor(Scope::Project).unwrap();

        assert_eq!(report.recovered_files, 1);
        assert_eq!(report.fixed_content_files, 1);

        // Verify content was normalized
        let content = store
            .backend
            .read_content(&orphan_id, Scope::Project)
            .unwrap()
            .unwrap();
        assert_eq!(content, "Title\n\nBody");
    }

    // --- Zombie Cleanup Tests ---

    #[test]
    fn test_doctor_removes_zombie_entries() {
        let backend = MemBackend::new();
        let zombie_id = Uuid::new_v4();

        // Create zombie: index entry exists but no content
        let mut index = std::collections::HashMap::new();
        index.insert(
            zombie_id,
            Metadata {
                id: zombie_id,
                created_at: Utc::now(),
                updated_at: Utc::now(),
                is_pinned: false,
                pinned_at: None,
                is_deleted: false,
                deleted_at: None,
                delete_protected: false,
                parent_id: None,
                title: "Zombie".to_string(),
            },
        );
        backend.save_index(Scope::Project, &index).unwrap();

        let mut store = PadStore::with_backend(backend);
        let report = store.doctor(Scope::Project).unwrap();

        assert_eq!(report.fixed_missing_files, 1);
        assert_eq!(report.recovered_files, 0);

        // Verify it's no longer in the store
        let result = store.get_pad(&zombie_id, Scope::Project);
        assert!(result.is_err());
    }

    // --- Staleness Detection Tests ---

    #[test]
    fn test_sync_updates_stale_metadata() {
        let mut store = make_store();

        // Create a pad normally
        let pad = Pad::new("Original Title".to_string(), "Content".to_string());
        let pad_id = pad.metadata.id;
        store.save_pad(&pad, Scope::Project).unwrap();

        // Simulate external edit: update content and set mtime to future
        store
            .backend
            .write_content(&pad_id, Scope::Project, "New Title\n\nNew content")
            .unwrap();

        // Set mtime to be newer than the index's updated_at
        let future_time = Utc::now() + Duration::hours(1);
        store
            .backend
            .set_content_mtime(&pad_id, Scope::Project, future_time);

        // Sync should detect staleness and update
        store.sync(Scope::Project).unwrap();

        let updated = store.get_pad(&pad_id, Scope::Project).unwrap();
        assert_eq!(updated.metadata.title, "New Title");
    }

    #[test]
    fn test_sync_ignores_fresh_metadata() {
        let mut store = make_store();

        // Create a pad normally
        let pad = Pad::new("Original Title".to_string(), "Content".to_string());
        let pad_id = pad.metadata.id;
        store.save_pad(&pad, Scope::Project).unwrap();

        // Set mtime to be older (in the past)
        let past_time = Utc::now() - Duration::hours(1);
        store
            .backend
            .set_content_mtime(&pad_id, Scope::Project, past_time);

        // Sync should NOT read the content since mtime <= updated_at
        store.sync(Scope::Project).unwrap();

        let fetched = store.get_pad(&pad_id, Scope::Project).unwrap();
        assert_eq!(fetched.metadata.title, "Original Title");
    }

    // --- Garbage Collection Tests ---

    #[test]
    fn test_doctor_removes_empty_content() {
        let backend = MemBackend::new();
        let empty_id = Uuid::new_v4();

        // Create content with only whitespace
        backend
            .write_content(&empty_id, Scope::Project, "   \n\n   ")
            .unwrap();

        let mut store = PadStore::with_backend(backend);
        store.doctor(Scope::Project).unwrap();

        // Content should be deleted
        let content = store
            .backend
            .read_content(&empty_id, Scope::Project)
            .unwrap();
        assert!(content.is_none());

        // No pad should exist
        let pads = store.list_pads(Scope::Project).unwrap();
        assert!(pads.is_empty());
    }

    // --- Scope Isolation Tests ---

    #[test]
    fn test_scopes_are_isolated() {
        let mut store = make_store();

        let pad = Pad::new("Project Pad".to_string(), "".to_string());
        store.save_pad(&pad, Scope::Project).unwrap();

        let global_pad = Pad::new("Global Pad".to_string(), "".to_string());
        store.save_pad(&global_pad, Scope::Global).unwrap();

        let project_pads = store.list_pads(Scope::Project).unwrap();
        let global_pads = store.list_pads(Scope::Global).unwrap();

        assert_eq!(project_pads.len(), 1);
        assert_eq!(project_pads[0].metadata.title, "Project Pad");

        assert_eq!(global_pads.len(), 1);
        assert_eq!(global_pads[0].metadata.title, "Global Pad");
    }

    // --- Error Handling Tests ---

    #[test]
    fn test_save_fails_on_write_error() {
        let backend = MemBackend::new();
        backend.set_simulate_write_error(true);

        let mut store = PadStore::with_backend(backend);
        let pad = Pad::new("Test".to_string(), "Content".to_string());

        let result = store.save_pad(&pad, Scope::Project);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_nonexistent_pad_returns_error() {
        let store = make_store();
        let result = store.get_pad(&Uuid::new_v4(), Scope::Project);
        assert!(result.is_err());
    }

    #[test]
    fn test_delete_nonexistent_pad_returns_error() {
        let mut store = make_store();
        let result = store.delete_pad(&Uuid::new_v4(), Scope::Project);
        assert!(result.is_err());
    }

    // --- Basic CRUD Tests ---

    #[test]
    fn test_save_and_get_pad() {
        let mut store = make_store();

        let pad = Pad::new("My Title".to_string(), "My content".to_string());
        let pad_id = pad.metadata.id;

        store.save_pad(&pad, Scope::Project).unwrap();

        let retrieved = store.get_pad(&pad_id, Scope::Project).unwrap();
        assert_eq!(retrieved.metadata.title, "My Title");
        assert_eq!(retrieved.content, "My Title\n\nMy content");
    }

    #[test]
    fn test_delete_removes_pad() {
        let mut store = make_store();

        let pad = Pad::new("To Delete".to_string(), "".to_string());
        let pad_id = pad.metadata.id;

        store.save_pad(&pad, Scope::Project).unwrap();
        store.delete_pad(&pad_id, Scope::Project).unwrap();

        // Should be gone from index
        let result = store.get_pad(&pad_id, Scope::Project);
        assert!(result.is_err());

        // Should be gone from content
        let content = store.backend.read_content(&pad_id, Scope::Project).unwrap();
        assert!(content.is_none());
    }

    #[test]
    fn test_list_pads_triggers_sync() {
        let backend = MemBackend::new();
        let orphan_id = Uuid::new_v4();

        // Create orphan
        backend
            .write_content(&orphan_id, Scope::Project, "Orphan")
            .unwrap();

        let store = PadStore::with_backend(backend);

        // list_pads should trigger sync and find the orphan
        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "Orphan");
    }
}
