use super::{DataStore, DoctorReport};
use crate::error::{PadzError, Result};
use crate::model::{Metadata, Pad, Scope};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

pub struct FileStore {
    project_root: Option<PathBuf>,
    global_root: PathBuf,
    file_ext: String,
}

impl FileStore {
    pub fn new(project_root: Option<PathBuf>, global_root: PathBuf) -> Self {
        Self {
            project_root,
            global_root,
            file_ext: ".txt".to_string(),
        }
    }

    pub fn with_file_ext(mut self, ext: &str) -> Self {
        if ext.starts_with('.') {
            self.file_ext = ext.to_string();
        } else {
            self.file_ext = format!(".{}", ext);
        }
        self
    }

    pub fn file_ext(&self) -> &str {
        &self.file_ext
    }

    fn pad_filename(&self, id: &Uuid) -> String {
        format!("pad-{}{}", id, self.file_ext)
    }

    /// Find the pad file for a given ID, checking both configured extension and .txt fallback
    fn find_pad_file(&self, root: &Path, id: &Uuid) -> Option<PathBuf> {
        // First try the configured extension
        let path = root.join(self.pad_filename(id));
        if path.exists() {
            return Some(path);
        }

        // Fallback to .txt for backwards compatibility
        if self.file_ext != ".txt" {
            let txt_path = root.join(format!("pad-{}.txt", id));
            if txt_path.exists() {
                return Some(txt_path);
            }
        }

        None
    }

    fn ensure_dir(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            fs::create_dir_all(path).map_err(PadzError::Io)?;
        }
        Ok(())
    }

    fn get_store_path(&self, scope: Scope) -> Result<PathBuf> {
        let root = match scope {
            Scope::Project => self.project_root.as_ref().ok_or_else(|| {
                PadzError::Store("No project scope available (not in a git repo?)".to_string())
            })?,
            Scope::Global => &self.global_root,
        };
        Ok(root.clone())
    }

    fn load_metadata(&self, store_path: &Path) -> Result<HashMap<Uuid, Metadata>> {
        let data_file = store_path.join("data.json");
        if !data_file.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(data_file).map_err(PadzError::Io)?;
        let meta: HashMap<Uuid, Metadata> =
            serde_json::from_str(&content).map_err(PadzError::Serialization)?;
        Ok(meta)
    }

    fn save_metadata(&self, store_path: &Path, meta: &HashMap<Uuid, Metadata>) -> Result<()> {
        let data_file = store_path.join("data.json");
        let content = serde_json::to_string_pretty(meta).map_err(PadzError::Serialization)?;
        fs::write(data_file, content).map_err(PadzError::Io)?;
        Ok(())
    }

    pub fn sync(&self, scope: Scope) -> Result<()> {
        let root = self.get_store_path(scope)?;
        if !root.exists() {
            return Ok(());
        }

        let mut meta_map = self.load_metadata(&root)?;
        let mut changes = false;

        // 1. Identify valid files and sync their state
        let entries = fs::read_dir(&root).map_err(PadzError::Io)?;
        let mut found_ids = Vec::new();

        for entry in entries {
            let entry = entry.map_err(PadzError::Io)?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if file_name.starts_with("pad-") {
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let uuid_part = stem.strip_prefix("pad-").unwrap_or("");

                        if let Ok(id) = Uuid::parse_str(uuid_part) {
                            found_ids.push(id);

                            // Read file metadata for mtime
                            let sys_meta = fs::metadata(&path).map_err(PadzError::Io)?;
                            let modified: DateTime<Utc> =
                                sys_meta.modified().unwrap_or(SystemTime::now()).into();

                            // Read content if:
                            // a) Orphan (not in DB)
                            // b) File is newer than DB entry
                            let needs_read = match meta_map.get(&id) {
                                None => true,
                                Some(meta) => modified > meta.updated_at,
                            };

                            if needs_read {
                                let content_raw =
                                    fs::read_to_string(&path).unwrap_or_else(|_| String::new()); // Best effort read

                                // Check for empty/useless files
                                if content_raw.trim().is_empty() {
                                    // Delete empty file
                                    let _ = fs::remove_file(&path);
                                    if meta_map.remove(&id).is_some() {
                                        changes = true;
                                    }
                                    continue;
                                }

                                // Update/Add to DB
                                if let Some((title, _)) =
                                    crate::model::parse_pad_content(&content_raw)
                                {
                                    if let Some(meta) = meta_map.get_mut(&id) {
                                        // Update existing
                                        if meta.title != title || meta.updated_at != modified {
                                            meta.title = title;
                                            meta.updated_at = modified;
                                            changes = true;
                                        }
                                    } else {
                                        // New / Orphan
                                        let created: DateTime<Utc> =
                                            sys_meta.created().unwrap_or(SystemTime::now()).into();
                                        let new_meta = Metadata {
                                            id,
                                            created_at: created,
                                            updated_at: modified,
                                            is_pinned: false,
                                            pinned_at: None,
                                            is_deleted: false,
                                            deleted_at: None,
                                            delete_protected: false,
                                            parent_id: None,
                                            title,
                                        };
                                        meta_map.insert(id, new_meta);
                                        changes = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 2. Remove DB entries that have no files
        let db_ids: Vec<Uuid> = meta_map.keys().cloned().collect();
        for id in db_ids {
            if !found_ids.contains(&id) {
                // Determine if really missing? found_ids contains all visible files.
                // If it was in DB but not in found_ids, it is deleted.
                meta_map.remove(&id);
                changes = true;
            }
        }

        if changes {
            self.save_metadata(&root, &meta_map)?;
        }
        Ok(())
    }
}

impl DataStore for FileStore {
    fn save_pad(&mut self, pad: &Pad, scope: Scope) -> Result<()> {
        let root = self.get_store_path(scope)?;
        self.ensure_dir(&root)?;

        // 1. Update metadata index
        let mut meta_map = self.load_metadata(&root)?;
        meta_map.insert(pad.metadata.id, pad.metadata.clone());
        self.save_metadata(&root, &meta_map)?;

        // 2. Write content file with configured extension
        let path = root.join(self.pad_filename(&pad.metadata.id));
        fs::write(path, &pad.content).map_err(PadzError::Io)?;

        Ok(())
    }

    fn get_pad(&self, id: &Uuid, scope: Scope) -> Result<Pad> {
        let root = self.get_store_path(scope)?;

        // 1. Get metadata
        let meta_map = self.load_metadata(&root)?;
        let metadata = meta_map.get(id).ok_or(PadzError::PadNotFound(*id))?.clone();

        // 2. Read content (with fallback for old .txt files)
        let content = if let Some(path) = self.find_pad_file(&root, id) {
            fs::read_to_string(path).map_err(PadzError::Io)?
        } else {
            String::new()
        };

        Ok(Pad { metadata, content })
    }

    fn list_pads(&self, scope: Scope) -> Result<Vec<Pad>> {
        // Sync before listing to ensure we have the latest state from disk.
        // Errors are ignored to allow listing even if sync partially fails.
        let _ = self.sync(scope);

        let root = self.get_store_path(scope)?;
        if !root.exists() {
            return Ok(Vec::new());
        }

        let meta_map = self.load_metadata(&root)?;
        let mut pads = Vec::new();

        for (id, metadata) in meta_map {
            let content = if let Some(path) = self.find_pad_file(&root, &id) {
                fs::read_to_string(path).map_err(PadzError::Io)?
            } else {
                String::new()
            };

            pads.push(Pad { metadata, content });
        }

        Ok(pads)
    }

    fn delete_pad(&mut self, id: &Uuid, scope: Scope) -> Result<()> {
        let root = self.get_store_path(scope)?;

        // 1. Remove from metadata
        let mut meta_map = self.load_metadata(&root)?;
        if meta_map.remove(id).is_none() {
            return Err(PadzError::PadNotFound(*id));
        }
        self.save_metadata(&root, &meta_map)?;

        // 2. Delete file (check both extensions)
        if let Some(path) = self.find_pad_file(&root, id) {
            fs::remove_file(path).map_err(PadzError::Io)?;
        }

        Ok(())
    }

    fn get_pad_path(&self, id: &Uuid, scope: Scope) -> Result<PathBuf> {
        let root = self.get_store_path(scope)?;

        // Return existing file path if found, otherwise return path with configured extension
        if let Some(path) = self.find_pad_file(&root, id) {
            Ok(path)
        } else {
            Ok(root.join(self.pad_filename(id)))
        }
    }

    fn doctor(&mut self, scope: Scope) -> Result<DoctorReport> {
        // Doctor performs similar work to sync but returns a detailed report.
        // Kept separate from sync to provide explicit diagnostics.
        let root = self.get_store_path(scope)?;
        if !root.exists() {
            return Ok(DoctorReport::default());
        }

        let mut meta_map = self.load_metadata(&root)?;
        let mut report = DoctorReport::default();
        let mut changes = false;

        // 1. Fix missing content files
        let ids: Vec<Uuid> = meta_map.keys().cloned().collect();
        for id in ids {
            if self.find_pad_file(&root, &id).is_none() {
                meta_map.remove(&id);
                report.fixed_missing_files += 1;
                changes = true;
            }
        }

        // 2. Recover orphan files
        let entries = fs::read_dir(&root).map_err(PadzError::Io)?;
        for entry in entries {
            let entry = entry.map_err(PadzError::Io)?;
            let path = entry.path();
            if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                    if file_name.starts_with("pad-") {
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let uuid_part = stem.strip_prefix("pad-").unwrap_or("");

                        if let Ok(id) = Uuid::parse_str(uuid_part) {
                            if let std::collections::hash_map::Entry::Vacant(e) = meta_map.entry(id)
                            {
                                // Orphan found
                                let content_raw = fs::read_to_string(&path).unwrap_or_default();

                                if let Some((title, normalized_content)) =
                                    crate::model::parse_pad_content(&content_raw)
                                {
                                    // Fix content if needed
                                    if content_raw != normalized_content {
                                        if let Err(e) = fs::write(&path, &normalized_content) {
                                            eprintln!(
                                                "Failed to normalize orphan file {}: {}",
                                                path.display(),
                                                e
                                            );
                                        } else {
                                            report.fixed_content_files += 1;
                                        }
                                    }

                                    let meta = fs::metadata(&path).map_err(PadzError::Io)?;
                                    let created: DateTime<Utc> =
                                        meta.created().unwrap_or(SystemTime::now()).into();
                                    let modified: DateTime<Utc> =
                                        meta.modified().unwrap_or(SystemTime::now()).into();

                                    let metadata = Metadata {
                                        id,
                                        created_at: created,
                                        updated_at: modified,
                                        is_pinned: false,
                                        pinned_at: None,
                                        is_deleted: false,
                                        deleted_at: None,
                                        delete_protected: false,
                                        parent_id: None,
                                        title,
                                    };

                                    e.insert(metadata);
                                    report.recovered_files += 1;
                                    changes = true;
                                }
                            }
                        }
                    }
                }
            }
        }

        if changes {
            self.save_metadata(&root, &meta_map)?;
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pad;
    use tempfile::tempdir;

    #[test]
    fn test_doctor_fixes_missing_files() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let mut store = FileStore::new(Some(root.clone()), root.clone());

        // create a pad
        let pad = Pad::new("Lost".to_string(), "Content".to_string());
        store.save_pad(&pad, Scope::Project).unwrap();

        // Delete the file manually
        let pad_path = store
            .get_pad_path(&pad.metadata.id, Scope::Project)
            .unwrap();
        fs::remove_file(pad_path).unwrap();

        // Run doctor
        let report = store.doctor(Scope::Project).unwrap();
        assert_eq!(report.fixed_missing_files, 1);

        // Check DB
        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 0);
    }

    #[test]
    fn test_doctor_recovers_orphan_files() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let mut store = FileStore::new(Some(root.clone()), root.clone());
        store.ensure_dir(&root).unwrap();

        // Create an orphan file manually
        let id = Uuid::new_v4();
        let filename = format!("pad-{}.txt", id);
        fs::write(root.join(filename), "Orphan Title\nOrphan Content").unwrap();

        // Run doctor
        let report = store.doctor(Scope::Project).unwrap();
        assert_eq!(report.recovered_files, 1);

        // Check DB
        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "Orphan Title");
        assert_eq!(pads[0].metadata.id, id);
    }
}
