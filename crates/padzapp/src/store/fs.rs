use super::bucketed::BucketedStore;
use super::fs_backend::FsBackend;
use super::pad_store::PadStore;
use std::path::PathBuf;

pub type FileStore = BucketedStore<FsBackend>;

impl FileStore {
    /// Create a new bucketed file store from project/global root paths.
    ///
    /// Each bucket gets its own subdirectory:
    /// - `{root}/active/`  — active pads (data.json + pad-*.txt)
    /// - `{root}/archived/` — archived pads
    /// - `{root}/deleted/`  — deleted pads
    /// - `{root}/`          — scope-level files (tags.json, padz.toml)
    pub fn new_fs(project_root: Option<PathBuf>, global_root: PathBuf) -> Self {
        BucketedStore::new(
            FsBackend::new(
                project_root.as_ref().map(|r| r.join("active")),
                global_root.join("active"),
            ),
            FsBackend::new(
                project_root.as_ref().map(|r| r.join("archived")),
                global_root.join("archived"),
            ),
            FsBackend::new(
                project_root.as_ref().map(|r| r.join("deleted")),
                global_root.join("deleted"),
            ),
            // Tag backend at scope root (shared across buckets)
            FsBackend::new(project_root, global_root),
        )
    }

    pub fn with_file_ext(mut self, ext: &str) -> Self {
        self.active = PadStore::with_backend(self.active.backend.with_file_ext(ext));
        self.archived = PadStore::with_backend(self.archived.backend.with_file_ext(ext));
        self.deleted = PadStore::with_backend(self.deleted.backend.with_file_ext(ext));
        // tag_backend doesn't need file_ext (no content files)
        self
    }

    pub fn file_ext(&self) -> &str {
        self.active.backend.file_ext()
    }
}
