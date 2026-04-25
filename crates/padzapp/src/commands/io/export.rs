use crate::commands::metadata_schema::{Archive, PadEntry, TagRegistryEntry, SCHEMA_VERSION};
use crate::commands::{CmdMessage, CmdResult, NestingMode};
use crate::error::{PadzError, Result};
use crate::index::DisplayIndex;
use crate::index::DisplayPad;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::{Bucket, DataStore};
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use pulldown_cmark_to_cmark::cmark;
use std::collections::HashSet;
use std::fs::File;
use std::io::Write;
use uuid::Uuid;

use crate::commands::helpers::{
    collect_nested_pads, indexed_pads, pads_by_selectors, NestedPad, TitleBucket,
};

/// Format for single-file export, determined by file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SingleFileFormat {
    Text,
    Markdown,
}

impl SingleFileFormat {
    /// Detect format from filename extension.
    pub fn from_filename(filename: &str) -> Self {
        let lower = filename.to_lowercase();
        if lower.ends_with(".md") || lower.ends_with(".markdown") {
            SingleFileFormat::Markdown
        } else {
            SingleFileFormat::Text
        }
    }
}

/// Result of a single-file export operation.
#[derive(Debug)]
pub struct SingleFileExportResult {
    pub content: String,
    pub format: SingleFileFormat,
}

pub fn run<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    nesting: NestingMode,
    with_metadata: bool,
) -> Result<CmdResult> {
    // 1. Resolve pads
    let pads = resolve_pads(store, scope, selectors)?;

    if pads.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to export."));
        return Ok(res);
    }

    let nested = resolve_nested(store, scope, &pads, nesting)?;

    // 2. Prepare output file
    let now = Utc::now();
    let suffix = if with_metadata { "meta" } else { "tar" };
    let filename = format!("padz-{}.{}.gz", now.format("%Y-%m-%d_%H-%M-%S"), suffix);
    let file = File::create(&filename).map_err(PadzError::Io)?;

    // 3. Write archive
    let mut result = CmdResult::default();
    let messages = if with_metadata {
        write_archive_with_metadata(file, store, scope, &nested)?
    } else {
        write_archive(file, &nested)?;
        Vec::new()
    };
    for m in messages {
        result.add_message(m);
    }
    result.add_message(CmdMessage::success(format!("Exported to {}", filename)));
    Ok(result)
}

fn resolve_nested<S: DataStore>(
    store: &S,
    scope: Scope,
    pads: &[DisplayPad],
    nesting: NestingMode,
) -> Result<Vec<NestedPad>> {
    match nesting {
        NestingMode::Flat => Ok(pads
            .iter()
            .map(|dp| NestedPad {
                pad: dp.clone(),
                depth: 0,
            })
            .collect()),
        NestingMode::Tree | NestingMode::Indented => collect_nested_pads(store, scope, pads),
    }
}

fn resolve_pads<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<Vec<DisplayPad>> {
    if selectors.is_empty() {
        Ok(indexed_pads(store, scope)?
            .into_iter()
            .filter(|dp| !matches!(dp.index, DisplayIndex::Deleted(_)))
            .collect())
    } else {
        pads_by_selectors(store, scope, selectors, false, TitleBucket::Any)
    }
}

fn write_archive<W: Write>(writer: W, pads: &[NestedPad]) -> Result<()> {
    let enc = GzEncoder::new(writer, Compression::default());
    let mut tar = tar::Builder::new(enc);

    for np in pads {
        let dp = &np.pad;
        let title = &dp.pad.metadata.title;
        let safe_title = sanitize_filename(title);
        let entry_name = format!(
            "padz/{}-{}.txt",
            safe_title,
            &dp.pad.metadata.id.to_string()[..8]
        );

        let content = format!("{}\n\n{}", title, dp.pad.content);

        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        tar.append_data(&mut header, entry_name, content.as_bytes())
            .map_err(PadzError::Io)?;
    }

    tar.finish().map_err(PadzError::Io)?;
    Ok(())
}

/// Write a tar.gz where each pad is stored in its native format with an
/// inline metadata header (md frontmatter / lex annotations). Pads in the
/// `.txt` format have no metadata dialect — they are exported without
/// metadata and counted into a single trailing warning.
pub(crate) fn write_archive_with_metadata<W: Write, S: DataStore>(
    writer: W,
    store: &S,
    scope: Scope,
    pads: &[NestedPad],
) -> Result<Vec<CmdMessage>> {
    use crate::commands::inline_metadata::{serialize_lex_metadata, serialize_md_frontmatter};

    let enc = GzEncoder::new(writer, Compression::default());
    let mut tar = tar::Builder::new(enc);

    let mut seen: HashSet<Uuid> = HashSet::new();
    let mut skipped_txt: Vec<String> = Vec::new();

    for np in pads {
        let dp = &np.pad;
        let meta = &dp.pad.metadata;
        if !seen.insert(meta.id) {
            continue;
        }

        // Source bucket: Active first, then Archived. Matches JSON export.
        let (bucket, source_path) = [Bucket::Active, Bucket::Archived]
            .iter()
            .find_map(|b| {
                store
                    .get_pad_path(&meta.id, scope, *b)
                    .ok()
                    .map(|p| (*b, p))
            })
            .unwrap_or((Bucket::Active, std::path::PathBuf::new()));

        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_else(|| "txt".to_string());

        let safe_title = sanitize_filename(&meta.title);
        let entry_name = format!("padz/{}-{}.{}", safe_title, &meta.id.to_string()[..8], ext);

        let metadata_block = match ext.as_str() {
            "md" | "markdown" => Some(serialize_md_frontmatter(meta, bucket)),
            "lex" => Some(serialize_lex_metadata(meta, bucket)),
            _ => {
                skipped_txt.push(meta.title.clone());
                None
            }
        };

        let content = match metadata_block {
            Some(block) => format!("{}{}", block, dp.pad.content),
            None => dp.pad.content.clone(),
        };

        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, entry_name, content.as_bytes())
            .map_err(PadzError::Io)?;
    }

    tar.finish().map_err(PadzError::Io)?;

    let mut messages = Vec::new();
    if !skipped_txt.is_empty() {
        let preview: Vec<&str> = skipped_txt.iter().take(3).map(String::as_str).collect();
        let suffix = if skipped_txt.len() > 3 {
            format!(" (+ {} more)", skipped_txt.len() - 3)
        } else {
            String::new()
        };
        messages.push(CmdMessage::warning(format!(
            "{} .txt pad(s) exported without metadata (txt has no metadata format): {}{}",
            skipped_txt.len(),
            preview.join(", "),
            suffix,
        )));
    }
    Ok(messages)
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim()
        .to_string()
}

/// Run single-file export, returning structured result.
pub fn run_single_file<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    title: &str,
    nesting: NestingMode,
) -> Result<CmdResult> {
    let pads = resolve_pads(store, scope, selectors)?;

    if pads.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to export."));
        return Ok(res);
    }

    let nested = resolve_nested(store, scope, &pads, nesting)?;

    let format = SingleFileFormat::from_filename(title);
    let result = merge_pads_to_single_file(&nested, title, format);

    // Write to file
    let filename = sanitize_output_filename(title, format);
    std::fs::write(&filename, &result.content).map_err(PadzError::Io)?;

    let mut cmd_result = CmdResult::default();
    cmd_result.add_message(CmdMessage::success(format!(
        "Exported {} pads to {}",
        pads.len(),
        filename
    )));
    Ok(cmd_result)
}

/// Merge pads into a single file content string.
pub fn merge_pads_to_single_file(
    pads: &[NestedPad],
    title: &str,
    format: SingleFileFormat,
) -> SingleFileExportResult {
    let content = match format {
        SingleFileFormat::Text => merge_as_text(pads),
        SingleFileFormat::Markdown => merge_as_markdown(pads, title),
    };
    SingleFileExportResult { content, format }
}

/// Merge pads as plain text with headers separating each file.
fn merge_as_text(pads: &[NestedPad]) -> String {
    let mut output = String::new();

    for (i, np) in pads.iter().enumerate() {
        let dp = &np.pad;
        if i > 0 {
            output.push_str("\n\n");
        }

        let indent = "    ".repeat(np.depth);

        // Add header with pad title
        let title = &dp.pad.metadata.title;
        let separator = "=".repeat(title.len().max(40));
        output.push_str(&indent);
        output.push_str(&separator);
        output.push('\n');
        output.push_str(&indent);
        output.push_str(title);
        output.push('\n');
        output.push_str(&indent);
        output.push_str(&separator);
        output.push_str("\n\n");

        // Add pad content (skip the title line since we already printed it)
        let content = &dp.pad.content;
        if let Some(body_start) = content.find("\n\n") {
            let body = content[body_start + 2..].trim();
            if !indent.is_empty() {
                for line in body.lines() {
                    if line.is_empty() {
                        output.push('\n');
                    } else {
                        output.push_str(&indent);
                        output.push_str(line);
                        output.push('\n');
                    }
                }
                // Remove trailing newline to match original behavior
                if output.ends_with('\n') && body.ends_with(|_: char| true) {
                    output.pop();
                }
            } else {
                output.push_str(body);
            }
        }
    }

    output
}

/// Merge pads as markdown with the export title as H1 and bumped headers.
fn merge_as_markdown(pads: &[NestedPad], export_title: &str) -> String {
    let mut output = String::new();

    // Export title as H1
    output.push_str("# ");
    output.push_str(export_title);
    output.push_str("\n\n");

    for (i, np) in pads.iter().enumerate() {
        let dp = &np.pad;
        if i > 0 {
            output.push_str("\n\n---\n\n");
        }

        // Pad title heading level based on depth: depth 0 = H2, depth 1 = H3, etc.
        let heading_level = (2 + np.depth).min(6);
        let hashes = "#".repeat(heading_level);
        output.push_str(&hashes);
        output.push(' ');
        output.push_str(&dp.pad.metadata.title);
        output.push_str("\n\n");

        // Get body content (skip title line)
        let content = &dp.pad.content;
        let body = if let Some(body_start) = content.find("\n\n") {
            content[body_start + 2..].trim()
        } else {
            ""
        };

        if !body.is_empty() {
            // Bump all headers in the body by (2 + depth) to nest under the pad heading
            let bumped = bump_markdown_headers_by(body, 2 + np.depth);
            output.push_str(&bumped);
        }
    }

    output
}

/// Bump all markdown header levels by 2 (H1->H3, H2->H4, etc., H6 stays H6).
/// Uses pulldown-cmark for proper markdown parsing.
pub fn bump_markdown_headers(content: &str) -> String {
    bump_markdown_headers_by(content, 2)
}

/// Bump all markdown header levels by `amount`, capped at H6.
pub fn bump_markdown_headers_by(content: &str, amount: usize) -> String {
    let options = Options::all();
    let parser = Parser::new_ext(content, options);

    let events: Vec<Event> = parser
        .map(|event| match event {
            Event::Start(Tag::Heading {
                level,
                id,
                classes,
                attrs,
            }) => {
                let new_level = bump_heading_level_by(level, amount);
                Event::Start(Tag::Heading {
                    level: new_level,
                    id,
                    classes,
                    attrs,
                })
            }
            Event::End(TagEnd::Heading(level)) => {
                let new_level = bump_heading_level_by(level, amount);
                Event::End(TagEnd::Heading(new_level))
            }
            other => other,
        })
        .collect();

    let mut output = String::new();
    cmark(events.iter(), &mut output).expect("cmark serialization failed");
    output
}

/// Bump a heading level by `amount`, capped at H6.
fn bump_heading_level_by(level: HeadingLevel, amount: usize) -> HeadingLevel {
    let current = match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    };
    let new = (current + amount).min(6);
    match new {
        1 => HeadingLevel::H1,
        2 => HeadingLevel::H2,
        3 => HeadingLevel::H3,
        4 => HeadingLevel::H4,
        5 => HeadingLevel::H5,
        _ => HeadingLevel::H6,
    }
}

/// Run JSON-format export: tar.gz containing raw pad files + `db.json` with
/// full metadata.
///
/// See [`crate::commands::metadata_schema`] for the archive format.
pub fn run_json<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    nesting: NestingMode,
) -> Result<CmdResult> {
    let pads = resolve_pads(store, scope, selectors)?;

    if pads.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to export."));
        return Ok(res);
    }

    let nested = resolve_nested(store, scope, &pads, nesting)?;

    let now = Utc::now();
    let filename = format!("padz-{}.json.tar.gz", now.format("%Y-%m-%d_%H-%M-%S"));
    let file = File::create(&filename).map_err(PadzError::Io)?;

    write_json_archive(file, store, scope, &nested, now)?;

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!("Exported to {}", filename)));
    Ok(result)
}

/// Collect the resolved + nested pads for a JSON export. Public for tests that
/// want to drive `write_json_archive` directly without writing to CWD.
#[cfg(test)]
pub(crate) fn collect_export_pads<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    nesting: NestingMode,
) -> Result<Vec<NestedPad>> {
    let pads = resolve_pads(store, scope, selectors)?;
    resolve_nested(store, scope, &pads, nesting)
}

pub(crate) fn write_json_archive<W: Write, S: DataStore>(
    writer: W,
    store: &S,
    scope: Scope,
    pads: &[NestedPad],
    exported_at: chrono::DateTime<Utc>,
) -> Result<()> {
    let enc = GzEncoder::new(writer, Compression::default());
    let mut tar = tar::Builder::new(enc);

    // 1. Write each pad file (raw content, preserving extension).
    //
    // Dedupe by UUID: pinned pads appear twice in the indexed tree (once with
    // a Pinned index, once with a Regular one). In the archive we want exactly
    // one file + db entry per pad.
    let mut pad_entries = Vec::with_capacity(pads.len());
    let mut referenced_tags: HashSet<String> = HashSet::new();
    let mut seen: HashSet<Uuid> = HashSet::new();

    for np in pads {
        let dp = &np.pad;
        let meta = &dp.pad.metadata;
        if !seen.insert(meta.id) {
            continue;
        }

        for t in &meta.tags {
            referenced_tags.insert(t.clone());
        }

        // Locate the pad in whichever bucket still holds it. Active first
        // (fast path), then Archived. Deleted is intentionally skipped:
        // resolve_pads filters deleted indexes out of the export set.
        let (bucket_name, source_path) = [Bucket::Active, Bucket::Archived]
            .iter()
            .find_map(|b| {
                store
                    .get_pad_path(&meta.id, scope, *b)
                    .ok()
                    .map(|p| (bucket_label(*b), p))
            })
            .unwrap_or_else(|| ("Active".to_string(), std::path::PathBuf::new()));

        let ext = source_path
            .extension()
            .and_then(|e| e.to_str())
            .map(str::to_owned)
            .unwrap_or_else(|| "txt".to_string());

        let file_name = format!("pads/pad-{}.{}", meta.id, ext);
        let entry_path = format!("padz/{}", file_name);
        let content_bytes = dp.pad.content.as_bytes();

        let mut header = tar::Header::new_gnu();
        header.set_size(content_bytes.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        tar.append_data(&mut header, entry_path, content_bytes)
            .map_err(PadzError::Io)?;

        let metadata_value = serde_json::to_value(meta)
            .map_err(|e| PadzError::Api(format!("Failed to serialize pad metadata: {}", e)))?;

        pad_entries.push(PadEntry {
            file: file_name,
            bucket: bucket_name,
            metadata: metadata_value,
        });
    }

    // 2. Collect the referenced subset of the tag registry.
    let all_tags = store.load_tags(scope).unwrap_or_default();
    let tags: Vec<TagRegistryEntry> = all_tags
        .into_iter()
        .filter(|t| referenced_tags.contains(&t.name))
        .map(|t| TagRegistryEntry {
            name: t.name,
            created_at: t.created_at,
        })
        .collect();

    // 3. Write db.json
    let archive = Archive {
        schema_version: SCHEMA_VERSION,
        exported_at,
        padz_version: env!("CARGO_PKG_VERSION").to_string(),
        pads: pad_entries,
        tags,
    };
    let json = serde_json::to_vec_pretty(&archive)
        .map_err(|e| PadzError::Api(format!("Failed to serialize archive: {}", e)))?;

    let mut header = tar::Header::new_gnu();
    header.set_size(json.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, "padz/db.json", json.as_slice())
        .map_err(PadzError::Io)?;

    tar.finish().map_err(PadzError::Io)?;
    Ok(())
}

fn bucket_label(b: Bucket) -> String {
    match b {
        Bucket::Active => "Active",
        Bucket::Archived => "Archived",
        Bucket::Deleted => "Deleted",
    }
    .to_string()
}

/// Generate output filename, ensuring proper extension.
fn sanitize_output_filename(title: &str, format: SingleFileFormat) -> String {
    let lower = title.to_lowercase();

    // Strip existing extension if present, then sanitize, then add correct extension
    let base_name = match format {
        SingleFileFormat::Markdown => {
            if lower.ends_with(".md") {
                &title[..title.len() - 3]
            } else if lower.ends_with(".markdown") {
                &title[..title.len() - 9]
            } else {
                title
            }
        }
        SingleFileFormat::Text => {
            if lower.ends_with(".txt") {
                &title[..title.len() - 4]
            } else {
                title
            }
        }
    };

    let sanitized_base = sanitize_filename(base_name);
    let ext = match format {
        SingleFileFormat::Markdown => "md",
        SingleFileFormat::Text => "txt",
    };

    format!("{}.{}", sanitized_base, ext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::index::{DisplayIndex, PadSelector};
    use crate::model::Scope;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn test_resolve_pads_exports_active_by_default() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Active".into(), "".into(), None).unwrap();

        let del_pad = crate::model::Pad::new("Deleted".into(), "".into());
        store
            .save_pad(&del_pad, Scope::Project, crate::store::Bucket::Deleted)
            .unwrap();

        let pads = resolve_pads(&store, Scope::Project, &[]).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].pad.metadata.title, "Active");
    }

    fn flat_nested(pads: &[DisplayPad]) -> Vec<NestedPad> {
        pads.iter()
            .map(|dp| NestedPad {
                pad: dp.clone(),
                depth: 0,
            })
            .collect()
    }

    #[test]
    fn test_write_archive_produces_content() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Test".into(),
            "Content".into(),
            None,
        )
        .unwrap();
        let pads = resolve_pads(&store, Scope::Project, &[]).unwrap();

        let mut buf = Vec::new();
        write_archive(&mut buf, &flat_nested(&pads)).unwrap();

        assert!(!buf.is_empty());
        // Could verify tar content but that requires untarring.
        // Checking header magic? Gzip header is 1f 8b
        assert_eq!(buf[0], 0x1f);
        assert_eq!(buf[1], 0x8b);
    }

    #[test]
    fn test_sanitize() {
        assert_eq!(sanitize_filename("Hello World"), "Hello World");
        assert_eq!(sanitize_filename("foo/bar"), "foo_bar");
        assert_eq!(sanitize_filename("baz\\qux"), "baz_qux");
    }

    #[test]
    fn test_format_detection() {
        assert_eq!(
            SingleFileFormat::from_filename("notes.md"),
            SingleFileFormat::Markdown
        );
        assert_eq!(
            SingleFileFormat::from_filename("notes.MD"),
            SingleFileFormat::Markdown
        );
        assert_eq!(
            SingleFileFormat::from_filename("notes.markdown"),
            SingleFileFormat::Markdown
        );
        assert_eq!(
            SingleFileFormat::from_filename("notes.txt"),
            SingleFileFormat::Text
        );
        assert_eq!(
            SingleFileFormat::from_filename("notes"),
            SingleFileFormat::Text
        );
        assert_eq!(
            SingleFileFormat::from_filename("My Notes"),
            SingleFileFormat::Text
        );
    }

    #[test]
    fn test_bump_markdown_headers_basic() {
        let input = "# Heading 1\n\nSome text\n\n## Heading 2\n\nMore text";
        let output = bump_markdown_headers(input);
        assert!(output.contains("### Heading 1"), "H1 should become H3");
        assert!(output.contains("#### Heading 2"), "H2 should become H4");
        assert!(output.contains("Some text"));
        assert!(output.contains("More text"));
    }

    #[test]
    fn test_bump_markdown_headers_caps_at_h6() {
        let input = "##### H5\n\n###### H6\n\nText";
        let output = bump_markdown_headers(input);
        // H5 -> H6, H6 stays H6
        assert!(output.contains("###### H5"), "H5 should become H6");
        // H6 stays H6 - both should be H6
        let h6_count = output.matches("######").count();
        assert_eq!(h6_count, 2, "Both headers should be H6");
    }

    #[test]
    fn test_bump_markdown_headers_h3_h4() {
        let input = "### H3 Header\n\nText\n\n#### H4 Header\n\nMore text";
        let output = bump_markdown_headers(input);
        // H3 -> H5, H4 -> H6
        assert!(output.contains("##### H3 Header"), "H3 should become H5");
        assert!(output.contains("###### H4 Header"), "H4 should become H6");
    }

    #[test]
    fn test_bump_markdown_headers_preserves_non_headers() {
        let input = "Regular paragraph\n\n- List item\n- Another item\n\n```rust\ncode\n```";
        let output = bump_markdown_headers(input);
        assert!(output.contains("Regular paragraph"));
        assert!(output.contains("List item"));
        assert!(output.contains("code"));
    }

    #[test]
    fn test_merge_as_text() {
        use crate::index::DisplayIndex;

        let pad1 = NestedPad {
            pad: DisplayPad {
                pad: crate::model::Pad::new("First Pad".into(), "Content one".into()),
                index: DisplayIndex::Regular(1),
                matches: None,
                children: vec![],
            },
            depth: 0,
        };
        let pad2 = NestedPad {
            pad: DisplayPad {
                pad: crate::model::Pad::new("Second Pad".into(), "Content two".into()),
                index: DisplayIndex::Regular(2),
                matches: None,
                children: vec![],
            },
            depth: 0,
        };

        let output = merge_as_text(&[pad1, pad2]);

        // Check headers are present
        assert!(output.contains("First Pad"));
        assert!(output.contains("Second Pad"));
        // Check separators (at least 40 =)
        assert!(output.contains("========================================"));
        // Check content
        assert!(output.contains("Content one"));
        assert!(output.contains("Content two"));
    }

    #[test]
    fn test_merge_as_markdown() {
        use crate::index::DisplayIndex;

        let pad1 = NestedPad {
            pad: DisplayPad {
                pad: crate::model::Pad::new(
                    "First Pad".into(),
                    "# Internal H1\n\nBody text".into(),
                ),
                index: DisplayIndex::Regular(1),
                matches: None,
                children: vec![],
            },
            depth: 0,
        };
        let pad2 = NestedPad {
            pad: DisplayPad {
                pad: crate::model::Pad::new(
                    "Second Pad".into(),
                    "## Internal H2\n\nMore body".into(),
                ),
                index: DisplayIndex::Regular(2),
                matches: None,
                children: vec![],
            },
            depth: 0,
        };

        let output = merge_as_markdown(&[pad1, pad2], "My Export");

        // Check export title is H1
        assert!(output.starts_with("# My Export"));
        // Check pad titles are H2
        assert!(output.contains("## First Pad"));
        assert!(output.contains("## Second Pad"));
        // Check internal headers are bumped (H1->H3, H2->H4)
        assert!(output.contains("### Internal H1"));
        assert!(output.contains("#### Internal H2"));
        // Check body content
        assert!(output.contains("Body text"));
        assert!(output.contains("More body"));
        // Check separator between pads
        assert!(output.contains("---"));
    }

    #[test]
    fn test_sanitize_output_filename() {
        assert_eq!(
            sanitize_output_filename("notes", SingleFileFormat::Markdown),
            "notes.md"
        );
        assert_eq!(
            sanitize_output_filename("notes.md", SingleFileFormat::Markdown),
            "notes.md"
        );
        assert_eq!(
            sanitize_output_filename("notes", SingleFileFormat::Text),
            "notes.txt"
        );
        assert_eq!(
            sanitize_output_filename("notes.txt", SingleFileFormat::Text),
            "notes.txt"
        );
        assert_eq!(
            sanitize_output_filename("my/notes", SingleFileFormat::Markdown),
            "my_notes.md"
        );
        // Test .markdown extension handling
        assert_eq!(
            sanitize_output_filename("notes.markdown", SingleFileFormat::Markdown),
            "notes.md"
        );
    }
    #[test]
    fn test_export_empty_does_nothing() {
        let store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        // No pads created
        let res = run(&store, Scope::Project, &[], NestingMode::Flat, false).unwrap();
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("No pads to export")));

        let res_single =
            run_single_file(&store, Scope::Project, &[], "out.md", NestingMode::Flat).unwrap();
        assert!(res_single
            .messages
            .iter()
            .any(|m| m.content.contains("No pads to export")));
    }

    #[test]
    fn test_export_single_file_creates_file() {
        use std::path::Path;
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Since run_single_file forces writing to CWD with a sanitized name,
        // we use a unique name to avoid collisions and check CWD.
        let unique_title = format!("Export_Test_{}", uuid::Uuid::new_v4());
        let expected_filename = format!("{}.md", unique_title); // sanitization should be no-op for alphanumeric+underscore
        let expected_path = Path::new(&expected_filename);

        // Export
        // Pass title with extension to trigger Markdown format detection,
        // but sanitization might duplicate extension if we are not careful?
        // sanitize_output_filename: if ends with .md and format is markdown, strips it, then adds .md.
        // So passing "Title.md" results in "Title.md".
        let input_title = format!("{}.md", unique_title);

        let res =
            run_single_file(&store, Scope::Project, &[], &input_title, NestingMode::Flat).unwrap();

        assert!(res.messages[0].content.contains("Exported 1 pads"));
        assert!(
            expected_path.exists(),
            "File {} should be created in CWD",
            expected_filename
        );

        let content = std::fs::read_to_string(expected_path).unwrap();
        assert!(content.contains(&format!("# {}", input_title))); // Title in H1
        assert!(content.contains("## A"));

        // Cleanup
        let _ = std::fs::remove_file(expected_path);
    }

    // --- Nesting mode tests ---

    #[test]
    fn test_merge_as_text_nested() {
        use crate::index::DisplayIndex;

        let pads = vec![
            NestedPad {
                pad: DisplayPad {
                    pad: crate::model::Pad::new("Parent".into(), "Parent body".into()),
                    index: DisplayIndex::Regular(1),
                    matches: None,
                    children: vec![],
                },
                depth: 0,
            },
            NestedPad {
                pad: DisplayPad {
                    pad: crate::model::Pad::new("Child".into(), "Child body".into()),
                    index: DisplayIndex::Regular(1),
                    matches: None,
                    children: vec![],
                },
                depth: 1,
            },
        ];

        let output = merge_as_text(&pads);

        // Parent header at depth 0 (no indent)
        assert!(output.contains("Parent"));
        assert!(output.contains("Parent body"));
        // Child header at depth 1 (4-space indent)
        assert!(output.contains("    Child"));
        assert!(output.contains("    Child body"));
    }

    #[test]
    fn test_merge_as_markdown_nested() {
        use crate::index::DisplayIndex;

        let pads = vec![
            NestedPad {
                pad: DisplayPad {
                    pad: crate::model::Pad::new("Parent".into(), "Parent body".into()),
                    index: DisplayIndex::Regular(1),
                    matches: None,
                    children: vec![],
                },
                depth: 0,
            },
            NestedPad {
                pad: DisplayPad {
                    pad: crate::model::Pad::new("Child".into(), "# H1 in child".into()),
                    index: DisplayIndex::Regular(1),
                    matches: None,
                    children: vec![],
                },
                depth: 1,
            },
        ];

        let output = merge_as_markdown(&pads, "Export");

        // H1 title
        assert!(output.starts_with("# Export"));
        // Parent at depth 0 -> H2
        assert!(output.contains("## Parent"));
        // Child at depth 1 -> H3
        assert!(output.contains("### Child"));
        // H1 in child body bumped by (2 + 1) = 3 -> H4
        assert!(output.contains("#### H1 in child"));
    }

    #[test]
    fn test_merge_as_text_nested_from_store() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Groceries".into(),
            "Weekly shopping".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Bread".into(),
            "Whole wheat".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let pads = resolve_pads(&store, Scope::Project, &[]).unwrap();
        let nested = resolve_nested(&store, Scope::Project, &pads, NestingMode::Tree).unwrap();

        let output = merge_as_text(&nested);

        // Parent present
        assert!(output.contains("Groceries"), "should contain parent title");
        assert!(
            output.contains("Weekly shopping"),
            "should contain parent body"
        );
        // Child present with indent (depth 1 = 4-space indent in text export)
        assert!(
            output.contains("    Bread"),
            "child title should be indented"
        );
        assert!(
            output.contains("    Whole wheat"),
            "child body should be indented"
        );
    }

    #[test]
    fn test_merge_as_markdown_nested_from_store() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Project".into(),
            "# Overview\n\nProject description".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Module A".into(),
            "## API\n\nEndpoints here".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let pads = resolve_pads(&store, Scope::Project, &[]).unwrap();
        let nested = resolve_nested(&store, Scope::Project, &pads, NestingMode::Tree).unwrap();

        let output = merge_as_markdown(&nested, "Docs");

        // Export title
        assert!(output.starts_with("# Docs"));
        // Parent at depth 0 -> H2
        assert!(output.contains("## Project"), "parent should be H2");
        // Child at depth 1 -> H3
        assert!(output.contains("### Module A"), "child should be H3");
        // Parent body H1 bumped by 2 -> H3
        assert!(
            output.contains("### Overview"),
            "parent body H1 should become H3"
        );
        // Child body H2 bumped by 3 (2+1) -> H5
        assert!(
            output.contains("##### API"),
            "child body H2 should become H5"
        );
    }

    #[test]
    fn test_flat_nesting_produces_no_children_in_export() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Parent".into(),
            "Parent content".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "Child content".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let pads = resolve_pads(&store, Scope::Project, &[]).unwrap();
        // Flat mode: should NOT include children
        let nested = resolve_nested(&store, Scope::Project, &pads, NestingMode::Flat).unwrap();

        // Only root-level pads (Parent) — no Child
        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].pad.pad.metadata.title, "Parent");
        assert_eq!(nested[0].depth, 0);
    }

    #[test]
    fn test_merge_as_markdown_deep_nesting_caps_at_h6() {
        use crate::index::DisplayIndex;

        let pads = vec![NestedPad {
            pad: DisplayPad {
                pad: crate::model::Pad::new("Deep".into(), "# Heading".into()),
                index: DisplayIndex::Regular(1),
                matches: None,
                children: vec![],
            },
            depth: 5, // depth 5 -> heading level 2+5=7 -> capped at 6
        }];

        let output = merge_as_markdown(&pads, "Export");

        // Title at depth 5 should cap at H6
        assert!(output.contains("###### Deep"));
    }
}
