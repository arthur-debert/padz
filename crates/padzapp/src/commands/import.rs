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
}
