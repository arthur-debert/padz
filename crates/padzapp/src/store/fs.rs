use super::fs_backend::FsBackend;
use super::pad_store::PadStore;
use std::path::PathBuf;

pub type FileStore = PadStore<FsBackend>;

impl FileStore {
    pub fn new(project_root: Option<PathBuf>, global_root: PathBuf) -> Self {
        let backend = FsBackend::new(project_root, global_root);
        PadStore::with_backend(backend)
    }

    pub fn with_file_ext(mut self, ext: &str) -> Self {
        self.backend = self.backend.with_file_ext(ext);
        self
    }

    pub fn file_ext(&self) -> &str {
        self.backend.file_ext()
    }
}
