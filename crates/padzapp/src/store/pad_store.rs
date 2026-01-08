use super::backend::StorageBackend;
use super::{DataStore, DoctorReport};
use crate::error::{PadzError, Result};
use crate::model::{Metadata, Pad, Scope};
use std::path::PathBuf;
use uuid::Uuid;

use std::time::SystemTime;

pub struct PadStore<B: StorageBackend> {
    pub backend: B,
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
