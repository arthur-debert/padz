use chrono::Utc;
use padzapp::model::{Metadata, Scope};
use padzapp::store::backend::StorageBackend;
use padzapp::store::fs_backend::FsBackend;
use padzapp::tags::TagEntry;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;
use uuid::Uuid;

fn setup() -> (TempDir, TempDir, FsBackend) {
    let project_dir = TempDir::new().unwrap();
    let global_dir = TempDir::new().unwrap();
    let backend = FsBackend::new(
        Some(project_dir.path().to_path_buf()),
        global_dir.path().to_path_buf(),
    );
    (project_dir, global_dir, backend)
}

#[test]
fn test_fs_backend_basic_content_io() {
    let (_proj, _glob, backend) = setup();
    let id = Uuid::new_v4();
    let scope = Scope::Project;

    // 1. Write
    backend.write_content(&id, scope, "Hello World").unwrap();

    // 2. Read
    let content = backend.read_content(&id, scope).unwrap();
    assert_eq!(content, Some("Hello World".to_string()));

    // 3. Delete
    backend.delete_content(&id, scope).unwrap();
    let content_after = backend.read_content(&id, scope).unwrap();
    assert_eq!(content_after, None);
}

#[test]
fn test_fs_backend_atomic_write_artifacts() {
    let (proj, _glob, backend) = setup();
    let id = Uuid::new_v4();
    let scope = Scope::Project;

    backend.write_content(&id, scope, "Atomic").unwrap();

    // Verify file exists
    let expected_path = proj.path().join(format!("pad-{}.txt", id));
    assert!(expected_path.exists());

    // Verify content on disk
    let on_disk = fs::read_to_string(&expected_path).unwrap();
    assert_eq!(on_disk, "Atomic");

    // Verify NO .tmp files are left behind
    let entries = fs::read_dir(proj.path()).unwrap();
    for entry in entries {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(!name.ends_with(".tmp"), "Found leftover tmp file: {}", name);
    }
}

#[test]
fn test_fs_backend_index_io() {
    let (_proj, _glob, backend) = setup();
    let scope = Scope::Project;

    let mut index = HashMap::new();
    let id = Uuid::new_v4();
    let meta = Metadata::new("Test Pad".to_string()); // Using helper just for struct, we overwrite ID
    let mut meta = meta;
    meta.id = id;

    index.insert(id, meta.clone());

    // Save
    backend.save_index(scope, &index).unwrap();

    // Load
    let loaded = backend.load_index(scope).unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded.get(&id).unwrap().title, "Test Pad");
}

#[test]
fn test_fs_backend_list_content_ids() {
    let (_proj, _glob, backend) = setup();
    let scope = Scope::Project;

    let id1 = Uuid::new_v4();
    let id2 = Uuid::new_v4();

    backend.write_content(&id1, scope, "1").unwrap();
    backend.write_content(&id2, scope, "2").unwrap();

    // Create a junk file to ensure it's ignored
    let proj_path = backend
        .content_path(&id1, scope)
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    fs::write(proj_path.join("junk.txt"), "ignore me").unwrap();
    fs::write(proj_path.join("pad-invalid-uuid.txt"), "ignore me too").unwrap();

    let ids = backend.list_content_ids(scope).unwrap();
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&id1));
    assert!(ids.contains(&id2));
}

#[test]
fn test_fs_backend_scope_isolation() {
    let (proj, glob, backend) = setup();

    let id = Uuid::new_v4();

    // Write to Project
    backend
        .write_content(&id, Scope::Project, "Project Content")
        .unwrap();

    // Write to Global
    backend
        .write_content(&id, Scope::Global, "Global Content")
        .unwrap();

    // Verify files in correct dirs
    assert!(proj.path().join(format!("pad-{}.txt", id)).exists());
    assert!(glob.path().join(format!("pad-{}.txt", id)).exists());

    // Read back
    assert_eq!(
        backend.read_content(&id, Scope::Project).unwrap(),
        Some("Project Content".to_string())
    );
    assert_eq!(
        backend.read_content(&id, Scope::Global).unwrap(),
        Some("Global Content".to_string())
    );
}

#[test]
fn test_fs_backend_custom_extension() {
    let (proj, _glob, backend) = setup();
    let backend = backend.with_file_ext(".md");

    let id = Uuid::new_v4();
    let scope = Scope::Project;

    backend.write_content(&id, scope, "Markdown").unwrap();

    let expected_path = proj.path().join(format!("pad-{}.md", id));
    assert!(expected_path.exists());

    let content = backend.read_content(&id, scope).unwrap();
    assert_eq!(content, Some("Markdown".to_string()));
}

#[test]
fn test_fs_backend_extension_fallback() {
    let (proj, _glob, backend) = setup();
    // Configured with .md
    let backend = backend.with_file_ext(".md");

    let id = Uuid::new_v4();
    let scope = Scope::Project;

    // Manually create a .txt file (legacy)
    let txt_path = proj.path().join(format!("pad-{}.txt", id));
    fs::write(&txt_path, "Legacy Content").unwrap();

    // Read should find it via fallback
    let content = backend.read_content(&id, scope).unwrap();
    assert_eq!(content, Some("Legacy Content".to_string()));

    // Find path should return .txt path
    let path = backend.content_path(&id, scope).unwrap();
    assert_eq!(path, txt_path);
}

#[test]
fn test_fs_backend_mtime() {
    let (_proj, _glob, backend) = setup();
    let id = Uuid::new_v4();
    let scope = Scope::Project;

    backend.write_content(&id, scope, "Time").unwrap();

    let mtime = backend.content_mtime(&id, scope).unwrap();
    assert!(mtime.is_some());
    // Sanity check: mtime should be close to now
    let diff = Utc::now().signed_duration_since(mtime.unwrap());
    assert!(diff.num_seconds().abs() < 5);
}

#[test]
fn test_fs_backend_project_scope_unavailable() {
    let global_dir = TempDir::new().unwrap();
    // Create backend with NO project root
    let backend = FsBackend::new(None, global_dir.path().to_path_buf());

    let id = Uuid::new_v4();

    // Trying to write to Project scope should fail
    let result = backend.write_content(&id, Scope::Project, "Content");
    assert!(result.is_err());

    // Trying to read from Project scope should fail
    let result = backend.read_content(&id, Scope::Project);
    assert!(result.is_err());

    // Trying to load index from Project scope should fail
    let result = backend.load_index(Scope::Project);
    assert!(result.is_err());

    // Global scope should still work
    backend
        .write_content(&id, Scope::Global, "Global Content")
        .unwrap();
    let content = backend.read_content(&id, Scope::Global).unwrap();
    assert_eq!(content, Some("Global Content".to_string()));
}

#[test]
fn test_fs_backend_scope_available() {
    let global_dir = TempDir::new().unwrap();
    let project_dir = TempDir::new().unwrap();

    // Backend with both scopes
    let backend_both = FsBackend::new(
        Some(project_dir.path().to_path_buf()),
        global_dir.path().to_path_buf(),
    );
    assert!(backend_both.scope_available(Scope::Project));
    assert!(backend_both.scope_available(Scope::Global));

    // Backend with only global scope
    let backend_global_only = FsBackend::new(None, global_dir.path().to_path_buf());
    assert!(!backend_global_only.scope_available(Scope::Project));
    assert!(backend_global_only.scope_available(Scope::Global));
}

#[test]
fn test_fs_backend_extension_without_dot() {
    let (proj, _glob, backend) = setup();
    // Use extension WITHOUT leading dot
    let backend = backend.with_file_ext("md");

    // Verify it normalizes to have dot
    assert_eq!(backend.file_ext(), ".md");

    let id = Uuid::new_v4();
    let scope = Scope::Project;

    backend.write_content(&id, scope, "Markdown").unwrap();

    // File should be created with .md extension
    let expected_path = proj.path().join(format!("pad-{}.md", id));
    assert!(expected_path.exists());
}

#[test]
fn test_fs_backend_read_nonexistent_file() {
    let (_proj, _glob, backend) = setup();
    let id = Uuid::new_v4();

    // Reading a file that doesn't exist should return None, not error
    let content = backend.read_content(&id, Scope::Project).unwrap();
    assert_eq!(content, None);
}

#[test]
fn test_fs_backend_delete_nonexistent_file() {
    let (_proj, _glob, backend) = setup();
    let id = Uuid::new_v4();

    // Deleting a file that doesn't exist should succeed silently
    let result = backend.delete_content(&id, Scope::Project);
    assert!(result.is_ok());
}

#[test]
fn test_fs_backend_mtime_nonexistent_file() {
    let (_proj, _glob, backend) = setup();
    let id = Uuid::new_v4();

    // mtime for nonexistent file should return None
    let mtime = backend.content_mtime(&id, Scope::Project).unwrap();
    assert!(mtime.is_none());
}

#[test]
fn test_fs_backend_content_path_nonexistent_returns_expected_path() {
    let (proj, _glob, backend) = setup();
    let id = Uuid::new_v4();

    // content_path for nonexistent file should return the expected path
    let path = backend.content_path(&id, Scope::Project).unwrap();
    assert_eq!(path, proj.path().join(format!("pad-{}.txt", id)));
}

#[test]
fn test_fs_backend_list_content_ids_empty_dir() {
    let (_proj, _glob, backend) = setup();

    // Listing an empty directory should return empty vec, not error
    let ids = backend.list_content_ids(Scope::Project).unwrap();
    assert!(ids.is_empty());
}

#[test]
fn test_fs_backend_list_content_ids_nonexistent_dir() {
    let global_dir = TempDir::new().unwrap();
    let nonexistent = global_dir.path().join("does_not_exist");

    let backend = FsBackend::new(Some(nonexistent), global_dir.path().to_path_buf());

    // Listing a nonexistent directory should return empty vec
    let ids = backend.list_content_ids(Scope::Project).unwrap();
    assert!(ids.is_empty());
}

#[test]
fn test_fs_backend_load_index_empty_dir() {
    let (_proj, _glob, backend) = setup();

    // Loading index from dir without data.json should return empty HashMap
    let index = backend.load_index(Scope::Project).unwrap();
    assert!(index.is_empty());
}

#[test]
fn test_fs_backend_tags_empty_dir() {
    let (_proj, _glob, backend) = setup();

    // Loading tags from dir without tags.json should return empty Vec
    let tags = backend.load_tags(Scope::Project).unwrap();
    assert!(tags.is_empty());
}

#[test]
fn test_fs_backend_tags_save_and_load() {
    let (proj, _glob, backend) = setup();

    let tags = vec![
        TagEntry::new("work".to_string()),
        TagEntry::new("rust".to_string()),
    ];

    // Save tags
    backend.save_tags(Scope::Project, &tags).unwrap();

    // Verify file was created
    let tags_file = proj.path().join("tags.json");
    assert!(tags_file.exists());

    // Load tags
    let loaded = backend.load_tags(Scope::Project).unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].name, "work");
    assert_eq!(loaded[1].name, "rust");
}

#[test]
fn test_fs_backend_tags_scope_isolation() {
    let (_proj, _glob, backend) = setup();

    let project_tags = vec![TagEntry::new("project-tag".to_string())];
    let global_tags = vec![TagEntry::new("global-tag".to_string())];

    backend.save_tags(Scope::Project, &project_tags).unwrap();
    backend.save_tags(Scope::Global, &global_tags).unwrap();

    // Load from each scope
    let loaded_project = backend.load_tags(Scope::Project).unwrap();
    let loaded_global = backend.load_tags(Scope::Global).unwrap();

    assert_eq!(loaded_project.len(), 1);
    assert_eq!(loaded_project[0].name, "project-tag");

    assert_eq!(loaded_global.len(), 1);
    assert_eq!(loaded_global[0].name, "global-tag");
}
