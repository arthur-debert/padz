use crate::store::fs::FileStore;
use std::path::PathBuf;
use tempfile::TempDir;

pub struct TestEnv {
    // We keep _temp_dir to ensure the directory is not dropped until the test is done
    pub _temp_dir: TempDir,
    pub store: FileStore,
    pub root: PathBuf,
}

impl Default for TestEnv {
    fn default() -> Self {
        Self::new()
    }
}

impl TestEnv {
    pub fn new() -> Self {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let root = temp_dir.path().to_path_buf();
        let store = FileStore::new(Some(root.clone()), root.clone());
        Self {
            _temp_dir: temp_dir,
            store,
            root,
        }
    }
}
