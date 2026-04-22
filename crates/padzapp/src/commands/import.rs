use crate::commands::inline_metadata::{parse_lex_metadata, parse_md_frontmatter};
use crate::commands::metadata_apply::{
    apply_metadata_defensively, parse_bucket_or_active, ParentPolicy,
};
use crate::commands::metadata_schema::{Archive, PadEntry};
use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::model::{parse_pad_content, Pad, Scope};
use crate::store::{Bucket, DataStore};
use crate::tags::TagEntry;
use flate2::read::GzDecoder;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    paths: Vec<PathBuf>,
    import_exts: &[String],
) -> Result<CmdResult> {
    let mut result = CmdResult::default();
    let mut imported_count = 0;

    for path in paths {
        if is_json_archive(&path) {
            match import_json_archive(store, scope, &path) {
                Ok((count, messages)) => {
                    imported_count += count;
                    result.add_message(CmdMessage::info(format!(
                        "Imported {} pads from {}",
                        count,
                        path.display()
                    )));
                    for m in messages {
                        result.add_message(m);
                    }
                }
                Err(e) => {
                    result.add_message(CmdMessage::warning(format!(
                        "Failed to import JSON archive {}: {}",
                        path.display(),
                        e
                    )));
                }
            }
            continue;
        }

        if path.is_dir() {
            // Import directory of plain files
            let entries = fs::read_dir(&path).map_err(PadzError::Io)?;
            for entry in entries {
                let entry = entry.map_err(PadzError::Io)?;
                let sub_path = entry.path();
                if sub_path.is_file() {
                    if let Some(ext) = sub_path.extension() {
                        let ext_str = format!(".{}", ext.to_string_lossy());
                        if import_exts.contains(&ext_str) {
                            if let Ok(res) = import_file(store, scope, &sub_path) {
                                imported_count += res.imported;
                                result.add_message(CmdMessage::info(format!(
                                    "Imported: {}",
                                    sub_path.display()
                                )));
                                for w in res.warnings {
                                    result.add_message(w);
                                }
                            }
                        }
                    }
                }
            }
        } else if path.is_file() {
            // Import file directly (try as text)
            match import_file(store, scope, &path) {
                Ok(res) => {
                    imported_count += res.imported;
                    result.add_message(CmdMessage::info(format!("Imported: {}", path.display())));
                    for w in res.warnings {
                        result.add_message(w);
                    }
                }
                Err(_) => {
                    result.add_message(CmdMessage::warning(format!(
                        "Failed to import: {}",
                        path.display()
                    )));
                }
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

/// Heuristic: `.tar.gz` / `.tgz` files are candidates for JSON archive import.
/// The actual detection (presence of `db.json`) is done by the importer.
fn is_json_archive(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    name.ends_with(".tar.gz") || name.ends_with(".tgz")
}

/// Result of importing a single file: pads created + any per-field warnings.
struct FileImportResult {
    imported: usize,
    warnings: Vec<CmdMessage>,
}

fn import_file<S: DataStore>(store: &mut S, scope: Scope, path: &Path) -> Result<FileImportResult> {
    let content_raw = fs::read_to_string(path).map_err(PadzError::Io)?;
    let ext = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    let label = path.display().to_string();
    import_content(store, scope, &content_raw, &ext, &label)
}

fn import_content<S: DataStore>(
    store: &mut S,
    scope: Scope,
    content_raw: &str,
    ext: &str,
    source_label: &str,
) -> Result<FileImportResult> {
    // Try inline metadata first, picking the dialect by file extension.
    let detected = match ext {
        "md" | "markdown" => parse_md_frontmatter(content_raw),
        "lex" => parse_lex_metadata(content_raw),
        _ => None,
    };

    if let Some((metadata_value, body)) = detected {
        // Body may still have leading whitespace; treat it as a raw pad doc.
        let Some((title, normalized)) = parse_pad_content(&body) else {
            return Ok(FileImportResult {
                imported: 0,
                warnings: Vec::new(),
            });
        };

        let mut pad = Pad::new(title.clone(), strip_title_from_body(&normalized, &title));
        let mut warnings = apply_metadata_defensively(
            &mut pad,
            &metadata_value,
            ParentPolicy::Trust,
            source_label,
        );

        if !title.is_empty() {
            pad.metadata.title = title;
        }

        // Bucket comes from metadata too; default to Active if missing.
        let bucket = metadata_value
            .get("bucket")
            .and_then(|v| v.as_str())
            .map(parse_bucket_or_active)
            .unwrap_or(Bucket::Active);

        store.save_pad(&pad, scope, bucket)?;

        if !warnings.is_empty() {
            warnings.insert(
                0,
                CmdMessage::info(format!("{}: applied inline metadata", source_label)),
            );
        }

        return Ok(FileImportResult {
            imported: 1,
            warnings,
        });
    }

    // No inline metadata — plain content path (backwards compatible).
    if let Some((title, body)) = crate::model::extract_title_and_body(content_raw) {
        crate::commands::create::run(store, scope, title, body, None)?;
        Ok(FileImportResult {
            imported: 1,
            warnings: Vec::new(),
        })
    } else {
        Ok(FileImportResult {
            imported: 0,
            warnings: Vec::new(),
        })
    }
}

/// Import a `.tar.gz` JSON archive.
///
/// Returns `(imported_count, metadata_warnings)`. The function is optimistic:
/// as long as the archive is readable, every pad file lands in the store.
/// Metadata that fails to parse becomes a per-field warning; the pad itself
/// always imports with at least a title and content.
fn import_json_archive<S: DataStore>(
    store: &mut S,
    scope: Scope,
    archive_path: &Path,
) -> Result<(usize, Vec<CmdMessage>)> {
    let file = fs::File::open(archive_path).map_err(PadzError::Io)?;
    let decoder = GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);

    // 1. Extract all entries into memory.
    //
    // Pads are small enough that streaming to disk first would add complexity
    // without meaningful savings. Keep everything keyed by archive-relative
    // path so we can cross-reference `db.json` against the pad files.
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    for entry in tar.entries().map_err(PadzError::Io)? {
        let mut entry = entry.map_err(PadzError::Io)?;
        let archive_path = entry
            .path()
            .map_err(PadzError::Io)?
            .to_string_lossy()
            .to_string();
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(PadzError::Io)?;
        files.insert(archive_path, buf);
    }

    // 2. Parse db.json. Accept either `padz/db.json` or bare `db.json`.
    let db_bytes = files
        .get("padz/db.json")
        .or_else(|| files.get("db.json"))
        .ok_or_else(|| PadzError::Api("Archive does not contain db.json".to_string()))?;

    let archive: Archive = serde_json::from_slice(db_bytes)
        .map_err(|e| PadzError::Api(format!("Invalid db.json: {}", e)))?;

    let mut warnings: Vec<CmdMessage> = Vec::new();
    let mut imported = 0usize;

    // 3. Index of UUIDs present in this archive, for parent_id orphaning.
    let archive_ids: HashSet<Uuid> = archive.pads.iter().filter_map(pad_id_from_entry).collect();

    // 4. Import each pad entry.
    for entry in &archive.pads {
        match import_pad_entry(store, scope, entry, &files, &archive_ids) {
            Ok((id, mut entry_warnings)) => {
                imported += 1;
                if !entry_warnings.is_empty() {
                    warnings.push(CmdMessage::info(format!(
                        "Pad {} imported; {} metadata warning(s)",
                        id,
                        entry_warnings.len()
                    )));
                    warnings.append(&mut entry_warnings);
                }
            }
            Err(e) => {
                warnings.push(CmdMessage::warning(format!(
                    "Skipping pad entry {}: {}",
                    entry.file, e
                )));
            }
        }
    }

    // 5. Merge referenced tag registry entries (don't overwrite existing).
    let existing_tags: HashMap<String, TagEntry> = store
        .load_tags(scope)
        .unwrap_or_default()
        .into_iter()
        .map(|t| (t.name.clone(), t))
        .collect();
    let mut new_tags: Vec<TagEntry> = existing_tags.values().cloned().collect();
    let mut added_tags = 0usize;
    for t in &archive.tags {
        if !existing_tags.contains_key(&t.name) {
            new_tags.push(TagEntry {
                name: t.name.clone(),
                created_at: t.created_at,
            });
            added_tags += 1;
        }
    }
    if added_tags > 0 {
        if let Err(e) = store.save_tags(scope, &new_tags) {
            warnings.push(CmdMessage::warning(format!(
                "Failed to merge tag registry: {}",
                e
            )));
        } else {
            warnings.push(CmdMessage::info(format!(
                "Merged {} tag registry entry/entries",
                added_tags
            )));
        }
    }

    Ok((imported, warnings))
}

fn pad_id_from_entry(entry: &PadEntry) -> Option<Uuid> {
    entry
        .metadata
        .get("id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
}

/// Import a single pad from an archive entry, applying metadata fields
/// defensively. The pad + content always lands; each metadata field is
/// optional and failures become warnings.
fn import_pad_entry<S: DataStore>(
    store: &mut S,
    scope: Scope,
    entry: &PadEntry,
    files: &HashMap<String, Vec<u8>>,
    archive_ids: &HashSet<Uuid>,
) -> Result<(Uuid, Vec<CmdMessage>)> {
    // Resolve file: db.json uses relative paths ("pads/pad-<uuid>.lex"); tar
    // entries include the "padz/" prefix.
    let content_bytes = files
        .get(&format!("padz/{}", entry.file))
        .or_else(|| files.get(&entry.file))
        .ok_or_else(|| PadzError::Api(format!("Missing file: {}", entry.file)))?;

    let raw = std::str::from_utf8(content_bytes)
        .map_err(|e| PadzError::Api(format!("Non-UTF-8 content: {}", e)))?;
    let (title, content) =
        parse_pad_content(raw).ok_or_else(|| PadzError::Api("Empty pad content".to_string()))?;

    // Start from a fresh Pad so we have a valid baseline, then overlay fields.
    let mut pad = Pad::new(title.clone(), strip_title_from_body(&content, &title));

    let warnings = apply_metadata_defensively(
        &mut pad,
        &entry.metadata,
        ParentPolicy::OrphanUnknown(archive_ids),
        &entry.file,
    );

    // The content's first line is the authoritative title — metadata.title may
    // be truncated to 60 chars, so prefer what we parsed out of the file.
    if !title.is_empty() {
        pad.metadata.title = title;
    }

    let bucket = parse_bucket_or_active(&entry.bucket);
    store.save_pad(&pad, scope, bucket)?;

    Ok((pad.metadata.id, warnings))
}

/// `parse_pad_content` returns `(title, "title\n\nbody")`. `Pad::new` expects
/// title + body separately (it re-normalizes). This helper extracts just the
/// body so `Pad::new` doesn't end up with a doubled title line.
pub(crate) fn strip_title_from_body(normalized: &str, title: &str) -> String {
    if let Some(rest) = normalized.strip_prefix(title) {
        rest.trim_start_matches('\n').trim_start().to_string()
    } else {
        normalized.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Scope;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    fn new_store() -> BucketedStore<MemBackend> {
        BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        )
    }

    fn import_content_simple<S: DataStore>(
        store: &mut S,
        scope: Scope,
        raw: &str,
    ) -> FileImportResult {
        import_content(store, scope, raw, "", "<test>").unwrap()
    }

    #[test]
    fn test_import_content_extracts_title() {
        let mut store = new_store();
        let raw = "My Title\nLine 1\nLine 2";
        import_content_simple(&mut store, Scope::Project, raw);

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "My Title");
        assert_eq!(pads[0].content, "My Title\n\nLine 1\nLine 2");
    }

    #[test]
    fn test_import_content_trims_leading_blanks() {
        let mut store = new_store();
        let raw = "Title\n\n\nReal Content";
        import_content_simple(&mut store, Scope::Project, raw);

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "Title");
        assert_eq!(pads[0].content, "Title\n\nReal Content");
    }

    #[test]
    fn test_import_from_directory() {
        let mut store = new_store();
        let temp_dir = tempfile::tempdir().unwrap();

        std::fs::write(temp_dir.path().join("note1.md"), "# Note 1\nContent 1").unwrap();
        std::fs::write(temp_dir.path().join("note2.txt"), "Note 2\n\nContent 2").unwrap();
        std::fs::write(temp_dir.path().join("ignored.foo"), "Ignored").unwrap();

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

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 2);
        assert!(pads.iter().any(|p| p.metadata.title == "# Note 1"));
        assert!(pads.iter().any(|p| p.metadata.title == "Note 2"));
    }

    #[test]
    fn test_import_file_directly() {
        let mut store = new_store();
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

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "# Single");
    }

    #[test]
    fn test_import_invalid_path() {
        let mut store = new_store();
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
        let mut store = new_store();
        let result = import_content_simple(&mut store, Scope::Project, "   \n\n  ");
        assert_eq!(result.imported, 0);

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 0);
    }

    #[test]
    fn test_import_file_with_non_utf8_fails() {
        let mut store = new_store();
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("binary.md");

        std::fs::write(&file_path, [0xFF, 0xFE, 0x00, 0x01]).unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            vec![file_path],
            &[".md".to_string()],
        )
        .unwrap();

        assert!(res.messages[0].content.contains("Failed to import"));
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 0")));
    }

    #[test]
    fn test_import_directory_skips_non_matching_extensions() {
        let mut store = new_store();
        let temp_dir = tempfile::tempdir().unwrap();

        std::fs::write(temp_dir.path().join("note.json"), r#"{"title": "Test"}"#).unwrap();
        std::fs::write(temp_dir.path().join("README"), "No Extension").unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            vec![temp_dir.path().to_path_buf()],
            &[".md".to_string()],
        )
        .unwrap();

        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 0")));
        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 0);
    }

    #[test]
    fn test_import_empty_paths_list() {
        let mut store = new_store();

        let res = run(&mut store, Scope::Project, vec![], &[".md".to_string()]).unwrap();

        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 0")));
        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 0);
    }

    #[test]
    fn test_import_multiple_paths_mixed_validity() {
        let mut store = new_store();
        let temp_dir = tempfile::tempdir().unwrap();

        let valid_file = temp_dir.path().join("valid.md");
        std::fs::write(&valid_file, "Valid Note\n\nContent").unwrap();

        let invalid_file = temp_dir.path().join("nonexistent.md");

        let res = run(
            &mut store,
            Scope::Project,
            vec![valid_file, invalid_file],
            &[".md".to_string()],
        )
        .unwrap();

        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Path not found")));
        assert!(res.messages.iter().any(|m| m.content.contains("Imported:")));

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 1);
    }

    #[test]
    fn test_import_directory_with_subdirs() {
        let mut store = new_store();
        let temp_dir = tempfile::tempdir().unwrap();

        let sub_dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        std::fs::write(sub_dir.join("nested.md"), "Nested\n\nContent").unwrap();

        std::fs::write(temp_dir.path().join("root.md"), "Root\n\nContent").unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            vec![temp_dir.path().to_path_buf()],
            &[".md".to_string()],
        )
        .unwrap();

        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "Root");
    }

    #[test]
    fn test_import_directory_file_with_empty_content() {
        let mut store = new_store();
        let temp_dir = tempfile::tempdir().unwrap();

        std::fs::write(temp_dir.path().join("empty.md"), "").unwrap();
        std::fs::write(temp_dir.path().join("whitespace.md"), "   \n\n   ").unwrap();
        std::fs::write(temp_dir.path().join("valid.md"), "Valid Title\n\nContent").unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            vec![temp_dir.path().to_path_buf()],
            &[".md".to_string()],
        )
        .unwrap();

        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "Valid Title");
    }

    // =========================================================================
    // Inline metadata (md/lex) tests
    // =========================================================================

    use crate::commands::inline_metadata::{serialize_lex_metadata, serialize_md_frontmatter};

    #[test]
    fn test_import_md_with_frontmatter_applies_metadata() {
        let mut store = new_store();

        // Build a pad, serialize its metadata as md frontmatter, write to a
        // file, and import it into a fresh store. Expect metadata preserved.
        let mut seed = crate::model::Pad::new("Alpha".into(), "Body text".into());
        seed.metadata.is_pinned = true;
        seed.metadata.delete_protected = true;
        seed.metadata.status = crate::model::TodoStatus::Done;
        seed.metadata.tags = vec!["work".into()];
        let expected_id = seed.metadata.id;

        let block = serialize_md_frontmatter(&seed.metadata, Bucket::Active);
        let body = format!("{}Alpha\n\nBody text", block);

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("alpha.md");
        std::fs::write(&path, body).unwrap();

        let res = run(&mut store, Scope::Project, vec![path], &[".md".into()]).unwrap();
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads.len(), 1);
        let p = &pads[0];
        assert_eq!(p.metadata.id, expected_id, "uuid preserved");
        assert_eq!(p.metadata.title, "Alpha");
        assert!(p.metadata.is_pinned);
        assert!(p.metadata.delete_protected);
        assert_eq!(p.metadata.status, crate::model::TodoStatus::Done);
        assert_eq!(p.metadata.tags, vec!["work".to_string()]);
        // Body should not contain the frontmatter
        assert!(!p.content.contains("padz.id"));
        assert!(!p.content.contains("---\n"));
    }

    #[test]
    fn test_import_lex_with_metadata_applies_metadata() {
        let mut store = new_store();

        let mut seed = crate::model::Pad::new("Beta".into(), "Some body".into());
        seed.metadata.status = crate::model::TodoStatus::InProgress;
        seed.metadata.tags = vec!["rust".into(), "cli".into()];
        let expected_id = seed.metadata.id;

        let block = serialize_lex_metadata(&seed.metadata, Bucket::Active);
        let body = format!("{}Beta\n\n    Some body", block);

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("beta.lex");
        std::fs::write(&path, body).unwrap();

        let res = run(&mut store, Scope::Project, vec![path], &[".lex".into()]).unwrap();
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads.len(), 1);
        let p = &pads[0];
        assert_eq!(p.metadata.id, expected_id, "uuid preserved");
        assert_eq!(p.metadata.title, "Beta");
        assert_eq!(p.metadata.status, crate::model::TodoStatus::InProgress);
        assert_eq!(p.metadata.tags, vec!["rust".to_string(), "cli".to_string()]);
        // Body should not contain the annotation block
        assert!(!p.content.contains(":: padz."));
    }

    #[test]
    fn test_import_md_without_frontmatter_falls_back_to_plain() {
        let mut store = new_store();

        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("plain.md");
        std::fs::write(&path, "# Heading\n\nBody").unwrap();

        run(&mut store, Scope::Project, vec![path], &[".md".into()]).unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "# Heading");
    }

    #[test]
    fn test_import_md_ignores_non_padz_frontmatter_keys() {
        let mut store = new_store();

        let body = "---\nauthor: Alice\ndate: 2026-01-01\n---\n\nTitle\n\nBody";
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("blog.md");
        std::fs::write(&path, body).unwrap();

        run(&mut store, Scope::Project, vec![path], &[".md".into()]).unwrap();

        // No padz.* keys -> treat as plain content; the frontmatter stays in
        // the body because our detector only fires when padz.* keys exist.
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads.len(), 1);
        // Title should still be "---" from the first line of the raw content
        // (we didn't extract frontmatter, so the title is "---")
        assert_eq!(pads[0].metadata.title, "---");
    }

    #[test]
    fn test_import_md_tolerates_invalid_status_field() {
        let mut store = new_store();

        let body = "---\npadz.id: \"11111111-2222-3333-4444-555555555555\"\npadz.status: NotAThing\n---\n\nTitle\n\nBody";
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("bad.md");
        std::fs::write(&path, body).unwrap();

        let res = run(&mut store, Scope::Project, vec![path], &[".md".into()]).unwrap();

        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("invalid status")));

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads.len(), 1);
        // uuid preserved, status fell back to default
        assert_eq!(
            pads[0].metadata.id.to_string(),
            "11111111-2222-3333-4444-555555555555"
        );
        assert_eq!(pads[0].metadata.status, crate::model::TodoStatus::Planned);
    }

    // =========================================================================
    // JSON archive tests
    // =========================================================================

    use crate::commands::export;
    use crate::commands::{create, NestingMode};
    use crate::index::{DisplayIndex, PadSelector};
    use crate::store::Bucket;
    use chrono::Utc;

    /// Build a JSON archive from the source store into a tempfile and return
    /// the path. Uses `write_json_archive` directly so the test controls the
    /// output location (the public `run_json` writes to CWD).
    fn export_to_tmpfile<S: crate::store::DataStore>(
        store: &S,
        scope: Scope,
        selectors: &[PadSelector],
    ) -> std::path::PathBuf {
        let nested =
            export::collect_export_pads(store, scope, selectors, NestingMode::Tree).unwrap();
        let temp = tempfile::NamedTempFile::with_suffix(".tar.gz").unwrap();
        let (file, path) = temp.keep().unwrap();
        export::write_json_archive(file, store, scope, &nested, Utc::now()).unwrap();
        path
    }

    #[test]
    fn test_json_roundtrip_preserves_metadata() {
        let mut src = new_store();
        create::run(
            &mut src,
            Scope::Project,
            "Alpha".into(),
            "Alpha body".into(),
            None,
        )
        .unwrap();

        // Customize metadata on the created pad
        let pad = src
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let pad_id = pad.metadata.id;
        let mut pad = pad;
        pad.metadata.is_pinned = true;
        pad.metadata.pinned_at = Some(Utc::now());
        pad.metadata.delete_protected = true;
        pad.metadata.status = crate::model::TodoStatus::Done;
        pad.metadata.tags = vec!["work".into()];
        src.save_pad(&pad, Scope::Project, Bucket::Active).unwrap();
        src.save_tags(Scope::Project, &[crate::tags::TagEntry::new("work".into())])
            .unwrap();

        let archive_path = export_to_tmpfile(&src, Scope::Project, &[]);

        // Fresh destination store
        let mut dst = new_store();
        let res = run(
            &mut dst,
            Scope::Project,
            vec![archive_path.clone()],
            &[".md".into(), ".txt".into(), ".lex".into()],
        )
        .unwrap();
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));

        let pads = dst.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads.len(), 1);
        let imported = &pads[0];
        assert_eq!(imported.metadata.id, pad_id, "uuid preserved");
        assert_eq!(imported.metadata.title, "Alpha");
        assert!(imported.metadata.is_pinned);
        assert!(imported.metadata.delete_protected);
        assert_eq!(imported.metadata.status, crate::model::TodoStatus::Done);
        assert_eq!(imported.metadata.tags, vec!["work"]);

        let tags = dst.load_tags(Scope::Project).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "work");

        std::fs::remove_file(archive_path).ok();
    }

    #[test]
    fn test_json_roundtrip_preserves_parent() {
        let mut src = new_store();
        create::run(
            &mut src,
            Scope::Project,
            "Parent".into(),
            "P body".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut src,
            Scope::Project,
            "Child".into(),
            "C body".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let parent_id = src
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .iter()
            .find(|p| p.metadata.title == "Parent")
            .unwrap()
            .metadata
            .id;

        let archive_path = export_to_tmpfile(&src, Scope::Project, &[]);

        let mut dst = new_store();
        run(
            &mut dst,
            Scope::Project,
            vec![archive_path.clone()],
            &[".md".into()],
        )
        .unwrap();

        let pads = dst.list_pads(Scope::Project, Bucket::Active).unwrap();
        let child = pads.iter().find(|p| p.metadata.title == "Child").unwrap();
        assert_eq!(child.metadata.parent_id, Some(parent_id));

        std::fs::remove_file(archive_path).ok();
    }

    #[test]
    fn test_json_orphans_child_when_parent_not_in_archive() {
        let mut src = new_store();
        create::run(
            &mut src,
            Scope::Project,
            "Parent".into(),
            "P body".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut src,
            Scope::Project,
            "Child".into(),
            "C body".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Export only the child (index 1.1 = Child) — parent is excluded
        let archive_path = export_to_tmpfile(
            &src,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        );

        let mut dst = new_store();
        run(
            &mut dst,
            Scope::Project,
            vec![archive_path.clone()],
            &[".md".into()],
        )
        .unwrap();

        let pads = dst.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.title, "Child");
        assert_eq!(
            pads[0].metadata.parent_id, None,
            "orphaned child should have no parent"
        );

        std::fs::remove_file(archive_path).ok();
    }

    #[test]
    fn test_json_import_tolerates_bad_metadata_field() {
        // Build a db.json by hand with a bogus `status`. Pad should still
        // import; status should fall back to default.
        let temp_dir = tempfile::tempdir().unwrap();
        let archive_path = temp_dir.path().join("bad.tar.gz");

        // Write the archive manually
        let file = std::fs::File::create(&archive_path).unwrap();
        let enc = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(enc);

        // Add pad file
        let pad_content = "Alpha\n\nBody";
        let mut h = tar::Header::new_gnu();
        h.set_size(pad_content.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        let uuid = uuid::Uuid::new_v4();
        let file_rel = format!("pads/pad-{}.txt", uuid);
        tar.append_data(&mut h, format!("padz/{}", file_rel), pad_content.as_bytes())
            .unwrap();

        // Add db.json with a broken status field and an unknown_field
        let db = format!(
            r#"{{
                "schema_version": 1,
                "exported_at": "2026-04-22T00:00:00Z",
                "padz_version": "1.3.0",
                "pads": [{{
                    "file": "{}",
                    "metadata": {{
                        "id": "{}",
                        "created_at": "2026-04-22T00:00:00Z",
                        "updated_at": "2026-04-22T00:00:00Z",
                        "is_pinned": false,
                        "pinned_at": null,
                        "delete_protected": false,
                        "parent_id": null,
                        "title": "Alpha",
                        "status": "NotARealStatus",
                        "tags": [],
                        "unknown_field": "future-padz"
                    }}
                }}],
                "tags": []
            }}"#,
            file_rel, uuid
        );
        let mut h = tar::Header::new_gnu();
        h.set_size(db.len() as u64);
        h.set_mode(0o644);
        h.set_cksum();
        tar.append_data(&mut h, "padz/db.json", db.as_bytes())
            .unwrap();

        tar.into_inner().unwrap().finish().unwrap();

        let mut dst = new_store();
        let res = run(
            &mut dst,
            Scope::Project,
            vec![archive_path.clone()],
            &[".md".into()],
        )
        .unwrap();
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));
        assert!(
            res.messages.iter().any(|m| m.content.contains("status")),
            "expected a warning mentioning the bad status field"
        );

        let pads = dst.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].metadata.id, uuid, "uuid preserved");
        // Bad status falls back to the default from Pad::new (Planned)
        assert_eq!(pads[0].metadata.status, crate::model::TodoStatus::Planned);
    }

    #[test]
    fn test_json_import_only_exports_referenced_tags() {
        let mut src = new_store();
        create::run(
            &mut src,
            Scope::Project,
            "Tagged".into(),
            "Body".into(),
            None,
        )
        .unwrap();

        // Source has 3 tags in the registry but only 1 referenced on the pad
        src.save_tags(
            Scope::Project,
            &[
                crate::tags::TagEntry::new("used".into()),
                crate::tags::TagEntry::new("unused1".into()),
                crate::tags::TagEntry::new("unused2".into()),
            ],
        )
        .unwrap();

        let pad = src
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .into_iter()
            .next()
            .unwrap();
        let mut pad = pad;
        pad.metadata.tags = vec!["used".into()];
        src.save_pad(&pad, Scope::Project, Bucket::Active).unwrap();

        let archive_path = export_to_tmpfile(&src, Scope::Project, &[]);

        let mut dst = new_store();
        run(
            &mut dst,
            Scope::Project,
            vec![archive_path.clone()],
            &[".md".into()],
        )
        .unwrap();

        let tags = dst.load_tags(Scope::Project).unwrap();
        let names: Vec<&str> = tags.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"used"));
        assert!(!names.contains(&"unused1"));
        assert!(!names.contains(&"unused2"));

        std::fs::remove_file(archive_path).ok();
    }

    #[test]
    fn test_import_file_with_no_extension() {
        let mut store = new_store();
        let temp_dir = tempfile::tempdir().unwrap();

        let file_path = temp_dir.path().join("NO_EXT");
        std::fs::write(&file_path, "Title Without Ext\n\nContent").unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            vec![file_path],
            &[".md".to_string()],
        )
        .unwrap();

        assert!(res.messages.iter().any(|m| m.content.contains("Imported:")));
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 1);
    }
}
