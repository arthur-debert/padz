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

    #[test]
    fn test_custom_file_extension() {
        use crate::test_utils::TestEnv;
        let env = TestEnv::new();
        // Use with_file_ext
        let mut store = env.store.with_file_ext("md");
        assert_eq!(store.file_ext(), ".md");

        // Save a pad
        let pad = Pad::new("Markdown Pad".to_string(), "# Content".to_string());
        store.save_pad(&pad, Scope::Project).unwrap();

        // Verify file exists with .md extension
        let path = store
            .get_pad_path(&pad.metadata.id, Scope::Project)
            .unwrap();
        assert!(path.to_str().unwrap().ends_with(".md"));
        assert!(path.exists());

        // Verify we can retrieve it
        let loaded = store.get_pad(&pad.metadata.id, Scope::Project).unwrap();
        assert_eq!(loaded.content, "Markdown Pad\n\n# Content");
    }

    #[test]
    fn test_extension_fallback() {
        use crate::test_utils::TestEnv;
        let env = TestEnv::new();
        let store = env.store.with_file_ext("md");

        // Manually create a .txt file (legacy)
        let id = Uuid::new_v4();
        let txt_path = env.root.join(format!("pad-{}.txt", id));
        fs::write(&txt_path, "Legacy Title\nLegacy Content").unwrap();

        // Manually add metadata so it's a valid pad in the system
        let mut meta = Metadata::new("Legacy Title".to_string());
        meta.id = id;
        store
            .save_metadata(
                &env.root,
                &std::collections::HashMap::from([(id, meta.clone())]),
            )
            .unwrap();

        // Try get_pad - should find the .txt file despite store being .md
        let loaded = store.get_pad(&id, Scope::Project).unwrap();
        assert_eq!(loaded.content, "Legacy Title\nLegacy Content");

        // list_pads should also work if we sync?
        // sync might be tricky because it lists directory.
        // Let's check finding path
        let found_path = store.get_pad_path(&id, Scope::Project).unwrap();
        assert_eq!(found_path, txt_path);
    }

    #[test]
    fn test_sync_lifecycle_updates_and_cleanup() {
        use crate::test_utils::TestEnv;
        let env = TestEnv::new();
        // root is reused from env directly or via store
        let mut store = env.store;

        // 1. Create a pad normally
        let pad = Pad::new("Initial".to_string(), "Content".to_string());
        store.save_pad(&pad, Scope::Project).unwrap();

        // 2. Modify file externally (simulating editor)
        let pad_path = store
            .get_pad_path(&pad.metadata.id, Scope::Project)
            .unwrap();
        // sleep to ensure mtime change
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&pad_path, "Updated Title\nUpdated Content").unwrap();

        // 3. Sync (via list_pads)
        let pads = store.list_pads(Scope::Project).unwrap();
        let updated_pad = pads
            .iter()
            .find(|p| p.metadata.id == pad.metadata.id)
            .unwrap();
        assert_eq!(updated_pad.metadata.title, "Updated Title");
        assert_eq!(updated_pad.content, "Updated Title\nUpdated Content");

        // 4. Empty the file externally
        // sleep to ensure mtime change
        std::thread::sleep(std::time::Duration::from_millis(10));
        fs::write(&pad_path, "   \n   ").unwrap(); // effectively empty

        // 5. Sync again - should delete the pad
        let pads_after = store.list_pads(Scope::Project).unwrap();
        assert!(pads_after.is_empty());
        assert!(!pad_path.exists());
    }

    #[test]
    fn test_zombie_cleanup() {
        use crate::test_utils::TestEnv;
        let env = TestEnv::new();
        let mut store = env.store;

        // Create pad
        let pad = Pad::new("To Delete".to_string(), "Content".to_string());
        store.save_pad(&pad, Scope::Project).unwrap();

        // Delete file manually
        let pad_path = store
            .get_pad_path(&pad.metadata.id, Scope::Project)
            .unwrap();
        fs::remove_file(pad_path).unwrap();

        // Sync
        let pads = store.list_pads(Scope::Project).unwrap();
        assert!(pads.is_empty());
    }

    #[test]
    fn test_scope_errors() {
        // Create store with NO project root
        let dir = tempdir().unwrap();
        let global_root = dir.path().to_path_buf();
        let mut store = FileStore::new(None, global_root);

        let pad = Pad::new("Test".to_string(), "Content".to_string());
        match store.save_pad(&pad, Scope::Project) {
            Err(PadzError::Store(msg)) => assert!(msg.contains("No project scope")),
            _ => panic!("Expected Store error for missing project scope"),
        }
    }

    #[test]
    fn test_not_found_errors() {
        use crate::test_utils::TestEnv;
        let env = TestEnv::new();
        let mut store = env.store;

        let id = Uuid::new_v4();
        // Delete non-existent
        match store.delete_pad(&id, Scope::Project) {
            Err(PadzError::PadNotFound(err_id)) => assert_eq!(err_id, id),
            _ => panic!("Expected PadNotFound"),
        }

        // Get non-existent
        match store.get_pad(&id, Scope::Project) {
            Err(PadzError::PadNotFound(err_id)) => assert_eq!(err_id, id),
            _ => panic!("Expected PadNotFound"),
        }
    }
}
