use padzapp::model::Scope;
use padzapp::store::fs::FileStore;
use tempfile::TempDir;

#[test]
fn test_filestore_wrapper_methods() {
    let proj = TempDir::new().unwrap();
    let glob = TempDir::new().unwrap();

    // Test new
    let store = FileStore::new(Some(proj.path().to_path_buf()), glob.path().to_path_buf());

    // Test config method delegtion
    let store = store.with_file_ext(".md");

    // Test getter delegation
    assert_eq!(store.file_ext(), ".md");

    // Verify it still works as a store
    let result = store.sync(Scope::Project);
    assert!(result.is_ok()); // Should succeed (empty)
}
