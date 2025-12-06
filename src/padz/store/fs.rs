use super::DataStore;
use crate::error::{PadzError, Result};
use crate::model::{Metadata, Pad, Scope};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
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
}
