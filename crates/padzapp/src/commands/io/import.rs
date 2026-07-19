//! Semantic import pipeline for plain files, directories, inline metadata,
//! and full-fidelity JSON archives.
//!
//! Every requested source produces one typed report. Recoverable source and
//! archive-entry failures remain local so independent inputs continue, while
//! metadata and tag-registry effects stay observable without authored prose.

use crate::commands::inline_metadata::{parse_lex_metadata, parse_md_frontmatter};
use crate::commands::metadata_apply::{
    apply_metadata_defensively, parse_bucket_or_active, MetadataApplicationWarning,
    MetadataWarningSeverity, ParentPolicy,
};
use crate::commands::metadata_schema::{Archive, PadEntry};
use crate::error::{PadzError, Result};
use crate::model::{parse_pad_content, Pad, Scope};
use crate::store::{Bucket, DataStore};
use crate::tags::TagEntry;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportStatus {
    FullSuccess,
    PartialSuccess,
    NoImports,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSourceKind {
    File,
    Directory,
    JsonArchive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSourceStatus {
    Imported,
    Skipped,
    Missing,
    Unreadable,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArchiveEntrySkipReason {
    MissingFile,
    InvalidEncoding,
    EmptyContent,
    StoreFailure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DirectoryEntrySkipReason {
    ReadEntry,
    InspectEntry,
    ImportFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TagRegistryMergeStatus {
    Merged,
    Failed,
}

/// Ordered, source-local facts emitted while importing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImportDiagnostic {
    InlineMetadataApplied {
        source_label: String,
        bucket: Bucket,
        warning_count: usize,
    },
    ArchiveMetadataSummary {
        pad_id: Uuid,
        warning_count: usize,
    },
    MetadataWarning {
        warning: MetadataApplicationWarning,
    },
    ArchiveEntrySkipped {
        entry: String,
        reason: ArchiveEntrySkipReason,
        detail: String,
    },
    DirectoryEntrySkipped {
        #[serde(skip_serializing_if = "Option::is_none")]
        entry: Option<PathBuf>,
        reason: DirectoryEntrySkipReason,
        detail: String,
    },
    TagRegistryMerge {
        status: TagRegistryMergeStatus,
        /// Number of registry entries successfully persisted by this merge.
        /// Failed merges always report zero.
        added: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

/// Report for one path explicitly requested by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportSourceReport {
    pub source: PathBuf,
    pub source_kind: ImportSourceKind,
    pub status: ImportSourceStatus,
    pub imported: usize,
    /// Plain files successfully read from this source, even when empty content
    /// caused the file to be skipped rather than imported.
    pub processed_files: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    pub diagnostics: Vec<ImportDiagnostic>,
}

/// Complete semantic import report, independent of any presentation client.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportReport {
    pub status: ImportStatus,
    pub total_imported: usize,
    pub sources: Vec<ImportSourceReport>,
}

/// Import every requested path and return one presentation-free report.
///
/// Recoverable directory-entry inspection and file-import failures are
/// retained as diagnostics so a partly imported directory cannot appear fully
/// successful.
pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    paths: Vec<PathBuf>,
    import_exts: &[String],
) -> Result<ImportReport> {
    let mut sources = Vec::with_capacity(paths.len());

    for path in paths {
        if is_json_archive(&path) {
            sources.push(match import_json_archive(store, scope, &path) {
                Ok(archive) => ImportSourceReport {
                    source: path,
                    source_kind: ImportSourceKind::JsonArchive,
                    status: if archive.imported > 0 {
                        ImportSourceStatus::Imported
                    } else {
                        ImportSourceStatus::Skipped
                    },
                    imported: archive.imported,
                    processed_files: Vec::new(),
                    detail: None,
                    diagnostics: archive.diagnostics,
                },
                Err(error) => ImportSourceReport {
                    source: path,
                    source_kind: ImportSourceKind::JsonArchive,
                    status: source_error_status(&error),
                    imported: 0,
                    processed_files: Vec::new(),
                    detail: Some(error.to_string()),
                    diagnostics: Vec::new(),
                },
            });
            continue;
        }

        if path.is_dir() {
            let entries = match fs::read_dir(&path) {
                Ok(entries) => entries,
                Err(error) => {
                    sources.push(unreadable_directory_report(path, error));
                    continue;
                }
            };
            let directory = import_directory_entries(
                store,
                scope,
                entries.map(|entry| entry.map(|entry| entry.path())),
                import_exts,
            );
            sources.push(ImportSourceReport {
                source: path,
                source_kind: ImportSourceKind::Directory,
                status: if directory.imported > 0 {
                    ImportSourceStatus::Imported
                } else {
                    ImportSourceStatus::Skipped
                },
                imported: directory.imported,
                processed_files: directory.processed_files,
                detail: None,
                diagnostics: directory.diagnostics,
            });
        } else if path.is_file() {
            sources.push(match import_file(store, scope, &path) {
                Ok(res) => {
                    let status = if res.imported > 0 {
                        ImportSourceStatus::Imported
                    } else {
                        ImportSourceStatus::Skipped
                    };
                    ImportSourceReport {
                        source: path.clone(),
                        source_kind: ImportSourceKind::File,
                        status,
                        imported: res.imported,
                        processed_files: vec![path],
                        detail: None,
                        diagnostics: res.diagnostics(),
                    }
                }
                Err(error) => ImportSourceReport {
                    source: path,
                    source_kind: ImportSourceKind::File,
                    status: source_error_status(&error),
                    imported: 0,
                    processed_files: Vec::new(),
                    detail: Some(error.to_string()),
                    diagnostics: Vec::new(),
                },
            });
        } else {
            sources.push(ImportSourceReport {
                source: path,
                source_kind: ImportSourceKind::File,
                status: ImportSourceStatus::Missing,
                imported: 0,
                processed_files: Vec::new(),
                detail: None,
                diagnostics: Vec::new(),
            });
        }
    }

    let total_imported = sources.iter().map(|source| source.imported).sum();
    let status = if total_imported == 0 {
        ImportStatus::NoImports
    } else if sources.iter().any(source_is_partial) {
        ImportStatus::PartialSuccess
    } else {
        ImportStatus::FullSuccess
    };
    Ok(ImportReport {
        status,
        total_imported,
        sources,
    })
}

fn unreadable_directory_report(path: PathBuf, error: std::io::Error) -> ImportSourceReport {
    let detail = error.to_string();
    let status = source_error_status(&PadzError::Io(error));
    ImportSourceReport {
        source: path,
        source_kind: ImportSourceKind::Directory,
        status,
        imported: 0,
        processed_files: Vec::new(),
        detail: Some(detail),
        diagnostics: Vec::new(),
    }
}

struct DirectoryImportResult {
    imported: usize,
    processed_files: Vec<PathBuf>,
    diagnostics: Vec<ImportDiagnostic>,
}

/// Import matching directory entries while retaining every recoverable entry
/// failure as an ordered diagnostic.
fn import_directory_entries<S, I>(
    store: &mut S,
    scope: Scope,
    entries: I,
    import_exts: &[String],
) -> DirectoryImportResult
where
    S: DataStore,
    I: IntoIterator<Item = std::io::Result<PathBuf>>,
{
    let mut imported = 0;
    let mut processed_files = Vec::new();
    let mut diagnostics = Vec::new();

    for entry in entries {
        let sub_path = match entry {
            Ok(path) => path,
            Err(error) => {
                diagnostics.push(ImportDiagnostic::DirectoryEntrySkipped {
                    entry: None,
                    reason: DirectoryEntrySkipReason::ReadEntry,
                    detail: error.to_string(),
                });
                continue;
            }
        };
        let Some(ext) = sub_path.extension() else {
            continue;
        };
        let ext = format!(".{}", ext.to_string_lossy());
        if !import_exts.contains(&ext) {
            continue;
        }
        match fs::metadata(&sub_path) {
            Ok(metadata) if metadata.is_file() => {}
            Ok(_) => continue,
            Err(error) => {
                diagnostics.push(ImportDiagnostic::DirectoryEntrySkipped {
                    entry: Some(sub_path),
                    reason: DirectoryEntrySkipReason::InspectEntry,
                    detail: error.to_string(),
                });
                continue;
            }
        }
        match import_file(store, scope, &sub_path) {
            Ok(result) => {
                imported += result.imported;
                processed_files.push(sub_path);
                diagnostics.extend(result.diagnostics());
            }
            Err(error) => diagnostics.push(ImportDiagnostic::DirectoryEntrySkipped {
                entry: Some(sub_path),
                reason: DirectoryEntrySkipReason::ImportFile,
                detail: error.to_string(),
            }),
        }
    }

    DirectoryImportResult {
        imported,
        processed_files,
        diagnostics,
    }
}

fn source_error_status(error: &PadzError) -> ImportSourceStatus {
    match error {
        PadzError::Io(error) if error.kind() != std::io::ErrorKind::InvalidData => {
            ImportSourceStatus::Unreadable
        }
        _ => ImportSourceStatus::Invalid,
    }
}

fn source_is_partial(source: &ImportSourceReport) -> bool {
    if source.status != ImportSourceStatus::Imported {
        return true;
    }
    source
        .diagnostics
        .iter()
        .any(|diagnostic| match diagnostic {
            ImportDiagnostic::MetadataWarning { warning } => {
                warning.severity == MetadataWarningSeverity::Warning
            }
            ImportDiagnostic::ArchiveEntrySkipped { .. }
            | ImportDiagnostic::DirectoryEntrySkipped { .. } => true,
            ImportDiagnostic::TagRegistryMerge { status, .. } => {
                *status == TagRegistryMergeStatus::Failed
            }
            ImportDiagnostic::InlineMetadataApplied { .. }
            | ImportDiagnostic::ArchiveMetadataSummary { .. } => false,
        })
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
    warnings: Vec<MetadataApplicationWarning>,
    inline_metadata: Option<(String, Bucket)>,
}

impl FileImportResult {
    fn diagnostics(self) -> Vec<ImportDiagnostic> {
        let mut diagnostics = Vec::new();
        if let Some((source_label, bucket)) = self.inline_metadata {
            diagnostics.push(ImportDiagnostic::InlineMetadataApplied {
                source_label,
                bucket,
                warning_count: self.warnings.len(),
            });
        }
        diagnostics.extend(
            self.warnings
                .into_iter()
                .map(|warning| ImportDiagnostic::MetadataWarning { warning }),
        );
        diagnostics
    }
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
                inline_metadata: None,
            });
        };

        let mut pad = Pad::new(title.clone(), strip_title_from_body(&normalized, &title));
        let warnings = apply_metadata_defensively(
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

        return Ok(FileImportResult {
            imported: 1,
            warnings,
            inline_metadata: Some((source_label.to_string(), bucket)),
        });
    }

    // No inline metadata — plain content path (backwards compatible).
    if let Some((title, body)) = crate::model::extract_title_and_body(content_raw) {
        crate::commands::create::run(store, scope, title, body, None)?;
        Ok(FileImportResult {
            imported: 1,
            warnings: Vec::new(),
            inline_metadata: None,
        })
    } else {
        Ok(FileImportResult {
            imported: 0,
            warnings: Vec::new(),
            inline_metadata: None,
        })
    }
}

/// Import a `.tar.gz` JSON archive.
///
/// Returns imported counts and ordered semantic diagnostics. The function is
/// optimistic: one bad entry does not prevent independent entries from landing.
fn import_json_archive<S: DataStore>(
    store: &mut S,
    scope: Scope,
    archive_path: &Path,
) -> Result<ArchiveImportResult> {
    let file = fs::File::open(archive_path).map_err(PadzError::Io)?;
    let decoder = GzDecoder::new(file);
    let mut tar = tar::Archive::new(decoder);

    // 1. Extract all entries into memory.
    //
    // Pads are small enough that streaming to disk first would add complexity
    // without meaningful savings. Keep everything keyed by archive-relative
    // path so we can cross-reference `db.json` against the pad files.
    let mut files: HashMap<String, Vec<u8>> = HashMap::new();
    for entry in tar
        .entries()
        .map_err(|error| PadzError::Api(format!("Invalid archive: {error}")))?
    {
        let mut entry =
            entry.map_err(|error| PadzError::Api(format!("Invalid archive: {error}")))?;
        let archive_path = entry
            .path()
            .map_err(|error| PadzError::Api(format!("Invalid archive path: {error}")))?
            .to_string_lossy()
            .to_string();
        let mut buf = Vec::new();
        entry
            .read_to_end(&mut buf)
            .map_err(|error| PadzError::Api(format!("Invalid archive entry: {error}")))?;
        files.insert(archive_path, buf);
    }

    // 2. Parse db.json. Accept either `padz/db.json` or bare `db.json`.
    let db_bytes = files
        .get("padz/db.json")
        .or_else(|| files.get("db.json"))
        .ok_or_else(|| PadzError::Api("Archive does not contain db.json".to_string()))?;

    let archive: Archive = serde_json::from_slice(db_bytes)
        .map_err(|e| PadzError::Api(format!("Invalid db.json: {}", e)))?;

    let mut diagnostics = Vec::new();
    let mut imported = 0usize;

    // 3. Index of UUIDs present in this archive, for parent_id orphaning.
    let archive_ids: HashSet<Uuid> = archive.pads.iter().filter_map(pad_id_from_entry).collect();

    // 4. Import each pad entry.
    for entry in &archive.pads {
        match import_pad_entry(store, scope, entry, &files, &archive_ids) {
            Ok((id, entry_warnings)) => {
                imported += 1;
                if !entry_warnings.is_empty() {
                    diagnostics.push(ImportDiagnostic::ArchiveMetadataSummary {
                        pad_id: id,
                        warning_count: entry_warnings.len(),
                    });
                    diagnostics.extend(
                        entry_warnings
                            .into_iter()
                            .map(|warning| ImportDiagnostic::MetadataWarning { warning }),
                    );
                }
            }
            Err(e) => {
                diagnostics.push(ImportDiagnostic::ArchiveEntrySkipped {
                    entry: entry.file.clone(),
                    reason: e.reason(),
                    detail: e.to_string(),
                });
            }
        }
    }

    // 5. Merge referenced tag registry entries (don't overwrite existing).
    let existing_tags: HashMap<String, TagEntry> = match store.load_tags(scope) {
        Ok(tags) => tags
            .into_iter()
            .map(|tag| (tag.name.clone(), tag))
            .collect(),
        Err(error) => {
            if !archive.tags.is_empty() {
                diagnostics.push(ImportDiagnostic::TagRegistryMerge {
                    status: TagRegistryMergeStatus::Failed,
                    added: 0,
                    detail: Some(error.to_string()),
                });
            }
            return Ok(ArchiveImportResult {
                imported,
                diagnostics,
            });
        }
    };
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
            diagnostics.push(ImportDiagnostic::TagRegistryMerge {
                status: TagRegistryMergeStatus::Failed,
                added: 0,
                detail: Some(e.to_string()),
            });
        } else {
            diagnostics.push(ImportDiagnostic::TagRegistryMerge {
                status: TagRegistryMergeStatus::Merged,
                added: added_tags,
                detail: None,
            });
        }
    } else if !archive.tags.is_empty() {
        diagnostics.push(ImportDiagnostic::TagRegistryMerge {
            status: TagRegistryMergeStatus::Merged,
            added: 0,
            detail: None,
        });
    }

    Ok(ArchiveImportResult {
        imported,
        diagnostics,
    })
}

struct ArchiveImportResult {
    imported: usize,
    diagnostics: Vec<ImportDiagnostic>,
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
) -> std::result::Result<(Uuid, Vec<MetadataApplicationWarning>), ArchiveEntryError> {
    // Resolve file: db.json uses relative paths ("pads/pad-<uuid>.lex"); tar
    // entries include the "padz/" prefix.
    let content_bytes = files
        .get(&format!("padz/{}", entry.file))
        .or_else(|| files.get(&entry.file))
        .ok_or_else(|| ArchiveEntryError::MissingFile(entry.file.clone()))?;

    let raw = std::str::from_utf8(content_bytes)
        .map_err(|error| ArchiveEntryError::InvalidEncoding(error.to_string()))?;
    let (title, content) = parse_pad_content(raw).ok_or(ArchiveEntryError::EmptyContent)?;

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
    store
        .save_pad(&pad, scope, bucket)
        .map_err(|error| ArchiveEntryError::StoreFailure(error.to_string()))?;

    Ok((pad.metadata.id, warnings))
}

#[derive(Debug, thiserror::Error)]
enum ArchiveEntryError {
    #[error("Missing file: {0}")]
    MissingFile(String),
    #[error("Non-UTF-8 content: {0}")]
    InvalidEncoding(String),
    #[error("Empty pad content")]
    EmptyContent,
    #[error("{0}")]
    StoreFailure(String),
}

impl ArchiveEntryError {
    fn reason(&self) -> ArchiveEntrySkipReason {
        match self {
            Self::MissingFile(_) => ArchiveEntrySkipReason::MissingFile,
            Self::InvalidEncoding(_) => ArchiveEntrySkipReason::InvalidEncoding,
            Self::EmptyContent => ArchiveEntrySkipReason::EmptyContent,
            Self::StoreFailure(_) => ArchiveEntrySkipReason::StoreFailure,
        }
    }
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

        assert_eq!(res.status, ImportStatus::FullSuccess);
        assert_eq!(res.total_imported, 2);
        assert_eq!(res.sources[0].source_kind, ImportSourceKind::Directory);
        assert_eq!(res.sources[0].processed_files.len(), 2);

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 2);
        assert!(pads.iter().any(|p| p.metadata.title == "# Note 1"));
        assert!(pads.iter().any(|p| p.metadata.title == "Note 2"));
    }

    #[test]
    fn unreadable_directory_report_retains_io_error_detail() {
        let report = unreadable_directory_report(
            PathBuf::from("locked"),
            std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "permission denied while listing",
            ),
        );

        assert_eq!(report.status, ImportSourceStatus::Unreadable);
        assert_eq!(
            report.detail.as_deref(),
            Some("permission denied while listing")
        );
    }

    #[test]
    fn directory_entry_failures_are_typed_and_make_import_partial() {
        let mut store = new_store();
        let temp_dir = tempfile::tempdir().unwrap();
        let imported_path = temp_dir.path().join("imported.md");
        let invalid_path = temp_dir.path().join("invalid.md");
        let vanished_path = temp_dir.path().join("vanished.md");
        std::fs::write(&imported_path, "Imported\n\nBody").unwrap();
        std::fs::write(&invalid_path, [0xff, 0xfe]).unwrap();

        let directory = import_directory_entries(
            &mut store,
            Scope::Project,
            vec![
                Ok(imported_path.clone()),
                Ok(invalid_path.clone()),
                Ok(vanished_path.clone()),
                Err(std::io::Error::other("entry vanished while listing")),
            ],
            &[".md".to_string()],
        );

        assert_eq!(directory.imported, 1);
        assert_eq!(directory.processed_files, vec![imported_path]);
        assert!(directory.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            ImportDiagnostic::DirectoryEntrySkipped {
                entry: Some(entry),
                reason: DirectoryEntrySkipReason::ImportFile,
                ..
            } if entry == &invalid_path
        )));
        assert!(directory.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            ImportDiagnostic::DirectoryEntrySkipped {
                entry: Some(entry),
                reason: DirectoryEntrySkipReason::InspectEntry,
                ..
            } if entry == &vanished_path
        )));
        assert!(directory.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            ImportDiagnostic::DirectoryEntrySkipped {
                entry: None,
                reason: DirectoryEntrySkipReason::ReadEntry,
                ..
            }
        )));
        assert!(source_is_partial(&ImportSourceReport {
            source: temp_dir.path().to_path_buf(),
            source_kind: ImportSourceKind::Directory,
            status: ImportSourceStatus::Imported,
            imported: directory.imported,
            processed_files: directory.processed_files,
            detail: None,
            diagnostics: directory.diagnostics,
        }));
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

        assert_eq!(res.status, ImportStatus::FullSuccess);
        assert_eq!(res.total_imported, 1);
        assert_eq!(res.sources[0].status, ImportSourceStatus::Imported);

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

        assert_eq!(res.status, ImportStatus::NoImports);
        assert_eq!(res.sources[0].status, ImportSourceStatus::Missing);
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

        assert_eq!(res.status, ImportStatus::NoImports);
        assert_eq!(res.sources[0].status, ImportSourceStatus::Invalid);
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

        assert_eq!(res.status, ImportStatus::NoImports);
        assert_eq!(res.sources[0].status, ImportSourceStatus::Skipped);
        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 0);
    }

    #[test]
    fn test_import_empty_paths_list() {
        let mut store = new_store();

        let res = run(&mut store, Scope::Project, vec![], &[".md".to_string()]).unwrap();

        assert_eq!(res.status, ImportStatus::NoImports);
        assert!(res.sources.is_empty());
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

        assert_eq!(res.status, ImportStatus::PartialSuccess);
        assert_eq!(res.total_imported, 1);
        assert_eq!(res.sources[0].status, ImportSourceStatus::Imported);
        assert_eq!(res.sources[1].status, ImportSourceStatus::Missing);

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

        assert_eq!(res.total_imported, 1);

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

        assert_eq!(res.total_imported, 1);

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
        assert_eq!(res.total_imported, 1);
        assert!(matches!(
            res.sources[0].diagnostics[0],
            ImportDiagnostic::InlineMetadataApplied { .. }
        ));

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
        assert_eq!(res.total_imported, 1);
        assert!(matches!(
            res.sources[0].diagnostics[0],
            ImportDiagnostic::InlineMetadataApplied { .. }
        ));

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

        assert_eq!(res.status, ImportStatus::PartialSuccess);
        assert!(res.sources[0].diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            ImportDiagnostic::MetadataWarning { warning }
                if warning.field.as_deref() == Some("status")
        )));

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
    /// the path. The core supplies owned bytes; this test supplies the final
    /// destination because the importer consumes a path.
    fn export_to_tmpfile<S: crate::store::DataStore>(
        store: &S,
        scope: Scope,
        selectors: &[PadSelector],
    ) -> std::path::PathBuf {
        use std::io::Write as _;

        let outcome = export::run_json(store, scope, selectors, NestingMode::Tree).unwrap();
        let export::ExportOutcome::Artifact(artifact) = outcome else {
            panic!("expected JSON export artifact");
        };
        assert_eq!(artifact.report.format, export::ExportFormat::JsonArchive);

        let mut temp = tempfile::NamedTempFile::with_suffix(".tar.gz").unwrap();
        temp.write_all(&artifact.bytes).unwrap();
        let (_, path) = temp.keep().unwrap();
        path
    }

    fn handcrafted_archive(db: serde_json::Value, files: &[(&str, &[u8])]) -> std::path::PathBuf {
        let temp = tempfile::NamedTempFile::with_suffix(".tar.gz").unwrap();
        let (file, path) = temp.keep().unwrap();
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut tar = tar::Builder::new(encoder);
        for (path, content) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            tar.append_data(&mut header, format!("padz/{path}"), *content)
                .unwrap();
        }
        let db = serde_json::to_vec(&db).unwrap();
        let mut header = tar::Header::new_gnu();
        header.set_size(db.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, "padz/db.json", db.as_slice())
            .unwrap();
        tar.into_inner().unwrap().finish().unwrap();
        path
    }

    #[test]
    fn archive_entry_skip_is_typed_and_other_entries_continue() {
        let good_id = Uuid::new_v4();
        let missing_id = Uuid::new_v4();
        let good_file = format!("pads/pad-{good_id}.txt");
        let missing_file = format!("pads/pad-{missing_id}.txt");
        let db = serde_json::json!({
            "schema_version": 1,
            "exported_at": "2026-04-22T00:00:00Z",
            "padz_version": "1.9.0",
            "pads": [
                {"file": good_file.clone(), "metadata": {"id": good_id.to_string()}},
                {"file": missing_file.clone(), "metadata": {"id": missing_id.to_string()}}
            ],
            "tags": []
        });
        let archive = handcrafted_archive(db, &[(good_file.as_str(), b"Good\n\nBody")]);
        let mut store = new_store();

        let report = run(
            &mut store,
            Scope::Project,
            vec![archive.clone()],
            &[".txt".into()],
        )
        .unwrap();

        assert_eq!(report.status, ImportStatus::PartialSuccess);
        assert_eq!(report.total_imported, 1);
        assert!(report.sources[0]
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(
                diagnostic,
                ImportDiagnostic::ArchiveEntrySkipped {
                    entry,
                    reason: ArchiveEntrySkipReason::MissingFile,
                    ..
                } if entry == &missing_file
            )));
        std::fs::remove_file(archive).ok();
    }

    #[test]
    fn tag_registry_merge_failure_is_typed_partial_success() {
        let id = Uuid::new_v4();
        let file = format!("pads/pad-{id}.txt");
        let db = serde_json::json!({
            "schema_version": 1,
            "exported_at": "2026-04-22T00:00:00Z",
            "padz_version": "1.9.0",
            "pads": [{"file": file.clone(), "metadata": {"id": id.to_string(), "tags": ["work"]}}],
            "tags": [{"name": "work", "created_at": "2026-04-22T00:00:00Z"}]
        });
        let archive = handcrafted_archive(db, &[(file.as_str(), b"Tagged\n\nBody")]);
        let mut store = new_store();
        store.tag_backend.set_simulate_write_error(true);

        let report = run(
            &mut store,
            Scope::Project,
            vec![archive.clone()],
            &[".txt".into()],
        )
        .unwrap();

        assert_eq!(report.status, ImportStatus::PartialSuccess);
        assert_eq!(report.total_imported, 1);
        assert!(report.sources[0]
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(
                diagnostic,
                ImportDiagnostic::TagRegistryMerge {
                    status: TagRegistryMergeStatus::Failed,
                    added: 0,
                    ..
                }
            )));
        std::fs::remove_file(archive).ok();
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
        assert_eq!(res.status, ImportStatus::FullSuccess);
        assert_eq!(res.total_imported, 1);
        assert!(res.sources[0].diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            ImportDiagnostic::TagRegistryMerge {
                status: TagRegistryMergeStatus::Merged,
                added: 1,
                ..
            }
        )));

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
        assert_eq!(res.status, ImportStatus::PartialSuccess);
        assert_eq!(res.total_imported, 1);
        assert!(res.sources[0].diagnostics.iter().any(|diagnostic| matches!(
            diagnostic,
            ImportDiagnostic::MetadataWarning { warning }
                if warning.field.as_deref() == Some("status")
        )));

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

        assert_eq!(res.status, ImportStatus::FullSuccess);
        assert_eq!(res.total_imported, 1);
        assert_eq!(res.sources[0].processed_files.len(), 1);

        let pads = store
            .list_pads(Scope::Project, crate::store::Bucket::Active)
            .unwrap();
        assert_eq!(pads.len(), 1);
    }
}
