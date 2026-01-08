use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::model::Scope;
use crate::store::DataStore;
use std::fs;
use std::path::{Path, PathBuf};

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    paths: Vec<PathBuf>,
    import_exts: &[String],
) -> Result<CmdResult> {
    let mut result = CmdResult::default();
    let mut imported_count = 0;

    for path in paths {
        if path.is_dir() {
            // Import directory
            let entries = fs::read_dir(&path).map_err(PadzError::Io)?;
            for entry in entries {
                let entry = entry.map_err(PadzError::Io)?;
                let sub_path = entry.path();
                if sub_path.is_file() {
                    if let Some(ext) = sub_path.extension() {
                        let ext_str = format!(".{}", ext.to_string_lossy());
                        if import_exts.contains(&ext_str) {
                            if let Ok(count) = import_file(store, scope, &sub_path) {
                                imported_count += count;
                                result.add_message(CmdMessage::info(format!(
                                    "Imported: {}",
                                    sub_path.display()
                                )));
                            }
                        }
                    }
                }
            }
        } else if path.is_file() {
            // Import file directly (try as text)
            if let Ok(count) = import_file(store, scope, &path) {
                imported_count += count;
                result.add_message(CmdMessage::info(format!("Imported: {}", path.display())));
            } else {
                result.add_message(CmdMessage::warning(format!(
                    "Failed to import: {}",
                    path.display()
                )));
            }
        } else {
            result.add_message(CmdMessage::warning(format!(
                "Path not found: {}",
                path.display()
            )));
        }
    }

    result.add_message(CmdMessage::success(format!(
        "Total imported: {}",
        imported_count
    )));
    Ok(result)
}

fn import_file<S: DataStore>(store: &mut S, scope: Scope, path: &Path) -> Result<usize> {
    let content_raw = fs::read_to_string(path).map_err(PadzError::Io)?;
    import_content(store, scope, &content_raw)
}

fn import_content<S: DataStore>(store: &mut S, scope: Scope, content_raw: &str) -> Result<usize> {
    if let Some((title, body)) = crate::model::extract_title_and_body(content_raw) {
        crate::commands::create::run(store, scope, title, body, None)?;
        Ok(1)
    } else {
        // Empty content, treated as ignore
        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn test_import_content_extracts_title() {
        let mut store = InMemoryStore::new();
        let raw = "My Title\nLine 1\nLine 2";
        import_content(&mut store, Scope::Project, raw).unwrap();

        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "My Title");
        assert_eq!(pads[0].content, "My Title\n\nLine 1\nLine 2");
    }

    #[test]
    fn test_import_content_trims_leading_blanks() {
        let mut store = InMemoryStore::new();
        let raw = "Title\n\n\nReal Content";
        import_content(&mut store, Scope::Project, raw).unwrap();

        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "Title");
        assert_eq!(pads[0].content, "Title\n\nReal Content");
    }

    #[test]
    fn test_import_from_directory() {
        let mut store = InMemoryStore::new();
        let temp_dir = tempfile::tempdir().unwrap();

        // Create valid files
        std::fs::write(temp_dir.path().join("note1.md"), "# Note 1\nContent 1").unwrap();
        std::fs::write(temp_dir.path().join("note2.txt"), "Note 2\n\nContent 2").unwrap();
        // Create ignored file
        std::fs::write(temp_dir.path().join("ignored.foo"), "Ignored").unwrap();

        // Run import on dir
        let res = run(
            &mut store,
            Scope::Project,
            vec![temp_dir.path().to_path_buf()],
            &[".md".to_string(), ".txt".to_string()],
        )
        .unwrap();

        assert_eq!(
            res.messages
                .iter()
                .filter(|m| m.content.contains("Imported:"))
                .count(),
            2
        );
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 2")));

        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 2);
        // Title extraction preserves markdown headers from first line
        assert!(pads.iter().any(|p| p.metadata.title == "# Note 1"));
        assert!(pads.iter().any(|p| p.metadata.title == "Note 2"));
    }

    #[test]
    fn test_import_file_directly() {
        let mut store = InMemoryStore::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("single.md");
        std::fs::write(&file_path, "# Single\nContent").unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            vec![file_path],
            &[".md".to_string()],
        )
        .unwrap();

        assert!(res.messages[0].content.contains("Imported:"));
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));

        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "# Single");
    }

    #[test]
    fn test_import_invalid_path() {
        let mut store = InMemoryStore::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let invalid_path = temp_dir.path().join("missing.md");

        let res = run(
            &mut store,
            Scope::Project,
            vec![invalid_path],
            &[".md".to_string()],
        )
        .unwrap();

        assert!(res.messages[0].content.contains("Path not found"));
    }

    #[test]
    fn test_import_empty_content_returns_zero() {
        let mut store = InMemoryStore::new();
        // Empty content (whitespace only) should not create a pad
        let result = import_content(&mut store, Scope::Project, "   \n\n  ").unwrap();
        assert_eq!(result, 0);

        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 0);
    }

    #[test]
    fn test_import_file_with_non_utf8_fails() {
        let mut store = InMemoryStore::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("binary.md");

        // Write invalid UTF-8 bytes
        std::fs::write(&file_path, [0xFF, 0xFE, 0x00, 0x01]).unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            vec![file_path],
            &[".md".to_string()],
        )
        .unwrap();

        // Should report failure
        assert!(res.messages[0].content.contains("Failed to import"));
        // Total should be 0
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 0")));
    }

    #[test]
    fn test_import_directory_skips_non_matching_extensions() {
        let mut store = InMemoryStore::new();
        let temp_dir = tempfile::tempdir().unwrap();

        // Create file with non-matching extension
        std::fs::write(temp_dir.path().join("note.json"), r#"{"title": "Test"}"#).unwrap();
        // Create file without extension
        std::fs::write(temp_dir.path().join("README"), "No Extension").unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            vec![temp_dir.path().to_path_buf()],
            &[".md".to_string()],
        )
        .unwrap();

        // Nothing imported
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 0")));
        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 0);
    }
}
