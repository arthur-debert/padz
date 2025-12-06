use super::DataStore;
use crate::error::{PadzError, Result};
use crate::model::{Metadata, Pad, Scope};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct JsonFileSystemStore {
    base_path: PathBuf,
}

impl JsonFileSystemStore {
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    fn get_scope_dir(&self, scope: Scope) -> PathBuf {
        // Simple layout:
        // base_path/project/ (for project scope? Or just base_path if base_path IS the project root?)
        // The spec says: ".padz directory. It's either in project's directory or uses the xdg data path".
        // So the `base_path` passed here should be the ROOT of the store (the .padz dir).
        // If we handle scopes by different Store instances, that's easier.
        // But the trait has `scope` in every method.
        // Option A: Store manages both scopes.
        // Option B: Store manages one directory, and the caller picks the right directory.
        // The Trait says `save_pad(..., scope)`. This implies the store handles scopes.
        // But local project scope is usually `./.padz` and global is `~/.local/share/padz`.
        // These are completely different paths.
        // A single `JsonFileSystemStore` initialized with ONE path cannot handle both effectively unless configured with both paths.
        // Let's assume `JsonFileSystemStore` is initialized with a map of Scope -> Path, or just handles one scope and we route in CLI?
        // CLI usually instantiates one store.
        // Let's modify `JsonFileSystemStore` to take a `Map<Scope, PathBuf>` or similar.
        // Or simpler: `JsonFileSystemStore` handles the path logic if we pass the root logic.
        // But Project path is dynamic.
        // Simpler approach:
        // `JsonFileSystemStore` struct holds `project_path: Option<PathBuf>` and `global_path: PathBuf`.

        // Let's stick to what's easiest for now:
        // The store trait expects usage like `store.list_pads(Scope::Global)`.
        match scope {
            Scope::Project => self.base_path.join("project"), // Placeholder if we use single root
            Scope::Global => self.base_path.join("global"),
        }
    }
}

// Rewind: usage.
// CLI: `padz list`. Auto-detects project root.
// If project root found: uses `.padz` in that root.
// If global flag: uses global XDG path.
// So we have TWO separate physical locations.
// A common pattern is `MultiStore` or `RouterStore`.
// Or `JsonFileSystemStore` simply needs to know both roots.

pub struct FileStore {
    project_root: Option<PathBuf>,
    global_root: PathBuf,
}

impl FileStore {
    pub fn new(project_root: Option<PathBuf>, global_root: PathBuf) -> Self {
        Self {
            project_root,
            global_root,
        }
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

        // 2. Write content file
        // Format: pad-{UUID}.txt
        let filename = format!("pad-{}.txt", pad.metadata.id);
        let path = root.join(filename);
        fs::write(path, &pad.content).map_err(PadzError::Io)?;

        Ok(())
    }

    fn get_pad(&self, id: &Uuid, scope: Scope) -> Result<Pad> {
        let root = self.get_store_path(scope)?;

        // 1. Get metadata
        let meta_map = self.load_metadata(&root)?;
        let metadata = meta_map.get(id).ok_or(PadzError::PadNotFound(*id))?.clone();

        // 2. Read content
        let filename = format!("pad-{}.txt", id);
        let path = root.join(filename);

        let content = if path.exists() {
            fs::read_to_string(path).map_err(PadzError::Io)?
        } else {
            String::new() // Should not happen if data integrity is kept, but maybe file deleted manually
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
            // Optimization: We could return Metadata only if we had a lighter struct,
            // but trait asks for Vec<Pad>. So we must read content.
            // Warning: This could be slow if many files.
            // PADZ.md limits to ~1000 pads. Reading 1000 small files is usually okay on SSD.
            // Ideally we'd lazy load content, but `Pad` struct has content.

            let filename = format!("pad-{}.txt", id);
            let path = root.join(filename);
            let content = if path.exists() {
                fs::read_to_string(path).map_err(PadzError::Io)?
            } else {
                String::new()
            };

            pads.push(Pad { metadata, content });
        }

        Ok(pads)
    }

    fn delete_pad(&mut self, id: &Uuid, scope: Scope) -> Result<()> {
        // Physical deletion (Flush)
        let root = self.get_store_path(scope)?;

        // 1. Remove from metadata
        let mut meta_map = self.load_metadata(&root)?;
        if meta_map.remove(id).is_none() {
            return Err(PadzError::PadNotFound(*id));
        }
        self.save_metadata(&root, &meta_map)?;

        // 2. Delete file
        let filename = format!("pad-{}.txt", id);
        let path = root.join(filename);
        if path.exists() {
            fs::remove_file(path).map_err(PadzError::Io)?;
        }

        Ok(())
    }

    fn get_pad_path(&self, id: &Uuid, scope: Scope) -> Result<PathBuf> {
        let root = self.get_store_path(scope)?;
        let filename = format!("pad-{}.txt", id);
        Ok(root.join(filename))
    }
}
