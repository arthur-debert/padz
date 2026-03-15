use super::backend::StorageBackend;
use crate::error::{PadzError, Result};
use crate::model::{Metadata, Scope};
use crate::tags::TagEntry;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use uuid::Uuid;

pub struct FsBackend {
    project_root: Option<PathBuf>,
    global_root: PathBuf,
    format: String,
}

impl FsBackend {
    pub fn new(project_root: Option<PathBuf>, global_root: PathBuf) -> Self {
        Self {
            project_root,
            global_root,
            format: ".txt".to_string(),
        }
    }

    pub fn with_format(mut self, ext: &str) -> Self {
        self.set_format(ext);
        self
    }

    pub fn set_format(&mut self, ext: &str) {
        if ext.starts_with('.') {
            self.format = ext.to_string();
        } else {
            self.format = format!(".{}", ext);
        }
    }

    pub fn format_ext(&self) -> &str {
        &self.format
    }

    fn pad_filename(&self, id: &Uuid) -> String {
        format!("pad-{}{}", id, self.format)
    }

    fn get_store_path_by_scope(&self, scope: Scope) -> Result<PathBuf> {
        let root = match scope {
            Scope::Project => self.project_root.as_ref().ok_or_else(|| {
                PadzError::Store("No project scope available (not in a git repo?)".to_string())
            })?,
            Scope::Global => &self.global_root,
        };
        Ok(root.clone())
    }

    fn ensure_dir(&self, path: &Path) -> Result<()> {
        if !path.exists() {
            fs::create_dir_all(path).map_err(PadzError::Io)?;
        }
        Ok(())
    }

    /// Find the pad file for a given UUID, regardless of its actual extension.
    ///
    /// Tries the configured format first (fast path), then scans the directory
    /// for any file matching `pad-{uuid}.*`. This supports mixed-format stores
    /// where different pads may have been created with different format settings.
    fn find_pad_file(&self, root: &Path, id: &Uuid) -> Option<PathBuf> {
        // 1. Try configured format (fast path)
        let path = root.join(self.pad_filename(id));
        if path.exists() {
            return Some(path);
        }

        // 2. Scan directory for any matching file (mixed-format support)
        let prefix = format!("pad-{}", id);
        if let Ok(entries) = fs::read_dir(root) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.starts_with(&prefix) && entry.path().is_file() {
                        return Some(entry.path());
                    }
                }
            }
        }
        None
    }
}

impl StorageBackend for FsBackend {
    fn load_index(&self, scope: Scope) -> Result<HashMap<Uuid, Metadata>> {
        let root = self.get_store_path_by_scope(scope)?;
        let data_file = root.join("data.json");
        if !data_file.exists() {
            return Ok(HashMap::new());
        }
        let content = fs::read_to_string(data_file).map_err(PadzError::Io)?;
        let meta: HashMap<Uuid, Metadata> =
            serde_json::from_str(&content).map_err(PadzError::Serialization)?;
        Ok(meta)
    }

    fn save_index(&self, scope: Scope, index: &HashMap<Uuid, Metadata>) -> Result<()> {
        let root = self.get_store_path_by_scope(scope)?;
        self.ensure_dir(&root)?;

        let data_file = root.join("data.json");
        let content = serde_json::to_string_pretty(index).map_err(PadzError::Serialization)?;

        // Atomic write for index too
        let tmp_file = root.join(format!(".data-{}.tmp", Uuid::new_v4()));
        fs::write(&tmp_file, content).map_err(PadzError::Io)?;
        fs::rename(&tmp_file, &data_file).map_err(PadzError::Io)?;

        Ok(())
    }

    fn load_tags(&self, scope: Scope) -> Result<Vec<TagEntry>> {
        let root = self.get_store_path_by_scope(scope)?;
        let tags_file = root.join("tags.json");
        if !tags_file.exists() {
            return Ok(Vec::new());
        }
        let content = fs::read_to_string(tags_file).map_err(PadzError::Io)?;
        let tags: Vec<TagEntry> =
            serde_json::from_str(&content).map_err(PadzError::Serialization)?;
        Ok(tags)
    }

    fn save_tags(&self, scope: Scope, tags: &[TagEntry]) -> Result<()> {
        let root = self.get_store_path_by_scope(scope)?;
        self.ensure_dir(&root)?;

        let tags_file = root.join("tags.json");
        let content = serde_json::to_string_pretty(tags).map_err(PadzError::Serialization)?;

        // Atomic write
        let tmp_file = root.join(format!(".tags-{}.tmp", Uuid::new_v4()));
        fs::write(&tmp_file, content).map_err(PadzError::Io)?;
        fs::rename(&tmp_file, &tags_file).map_err(PadzError::Io)?;

        Ok(())
    }

    fn read_content(&self, id: &Uuid, scope: Scope) -> Result<Option<String>> {
        let root = self.get_store_path_by_scope(scope)?;
        if let Some(path) = self.find_pad_file(&root, id) {
            let content = fs::read_to_string(path).map_err(PadzError::Io)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    fn write_content(&self, id: &Uuid, scope: Scope, content: &str) -> Result<()> {
        let root = self.get_store_path_by_scope(scope)?;
        self.ensure_dir(&root)?;

        // Use existing file path if pad already exists (preserves original extension).
        // Otherwise use configured format for new pads.
        let target_path = if let Some(existing) = self.find_pad_file(&root, id) {
            existing
        } else {
            root.join(self.pad_filename(id))
        };

        // Atomic Write
        let tmp_path = root.join(format!(".pad-{}.tmp", Uuid::new_v4()));
        fs::write(&tmp_path, content).map_err(PadzError::Io)?;
        fs::rename(&tmp_path, target_path).map_err(PadzError::Io)?;

        Ok(())
    }

    fn delete_content(&self, id: &Uuid, scope: Scope) -> Result<()> {
        let root = self.get_store_path_by_scope(scope)?;
        if let Some(path) = self.find_pad_file(&root, id) {
            fs::remove_file(path).map_err(PadzError::Io)?;
        }
        Ok(())
    }

    fn list_content_ids(&self, scope: Scope) -> Result<Vec<Uuid>> {
        let root = self.get_store_path_by_scope(scope)?;
        if !root.exists() {
            return Ok(Vec::new());
        }

        let mut ids = Vec::new();
        let entries = fs::read_dir(&root).map_err(PadzError::Io)?;

        for entry in entries {
            let entry = entry.map_err(PadzError::Io)?;
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if name.starts_with("pad-") {
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let uuid_part = stem.strip_prefix("pad-").unwrap_or("");
                        if let Ok(id) = Uuid::parse_str(uuid_part) {
                            ids.push(id);
                        }
                    }
                }
            }
        }
        Ok(ids)
    }

    fn content_mtime(&self, id: &Uuid, scope: Scope) -> Result<Option<DateTime<Utc>>> {
        let root = self.get_store_path_by_scope(scope)?;
        if let Some(path) = self.find_pad_file(&root, id) {
            let meta = fs::metadata(path).map_err(PadzError::Io)?;
            let modified: DateTime<Utc> = meta.modified().unwrap_or(SystemTime::now()).into();
            Ok(Some(modified))
        } else {
            Ok(None)
        }
    }

    fn content_path(&self, id: &Uuid, scope: Scope) -> Result<PathBuf> {
        let root = self.get_store_path_by_scope(scope)?;

        if let Some(path) = self.find_pad_file(&root, id) {
            Ok(path)
        } else {
            Ok(root.join(self.pad_filename(id)))
        }
    }

    fn scope_available(&self, scope: Scope) -> bool {
        self.get_store_path_by_scope(scope).is_ok()
    }
}
