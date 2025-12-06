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
    // "the first line is the title and subsequent ones are the content (if the first line of the content is a blank line(s trim it.))"

    let mut lines = content_raw.lines();
    let title = lines.next().unwrap_or("Untitled").trim().to_string();

    // Collect remaining lines.
    // Ensure we handle "first line is title".
    // "subsequent ones are the content".
    // "if the first line of the content is a blank line(s trim it.)"

    // We can collect remaining into a String (joined by newline)
    // Then trim ONLY leading newlines.

    // Or just skip while empty?
    let mut content_lines: Vec<&str> = lines.collect();

    // Trim leading blank lines
    while !content_lines.is_empty() && content_lines[0].trim().is_empty() {
        content_lines.remove(0);
    }

    let content = content_lines.join("\n");

    // Should we trim end too? Spec says trim first line blank lines.

    // Create pad
    // We need to use create logic.
    // Can we call api.create_pad? No we have store here.
    // Call commands::create::run or just store.save_pad?
    // commands::create::run properly wraps it and returns result.
    // But we are batching.
    // Let's create Pad and save.

    crate::commands::create::run(store, scope, title, content)?;
    Ok(1)
}
