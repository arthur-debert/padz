use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::index::DisplayIndex;
use crate::index::DisplayPad;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;
use flate2::write::GzEncoder;
use flate2::Compression;
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use pulldown_cmark_to_cmark::cmark;
use std::fs::File;
use std::io::Write;

use super::helpers::{indexed_pads, pads_by_selectors};

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

pub fn run<S: DataStore>(store: &S, scope: Scope, selectors: &[PadSelector]) -> Result<CmdResult> {
    // 1. Resolve pads
    let pads = resolve_pads(store, scope, selectors)?;

    if pads.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to export."));
        return Ok(res);
    }

    // 2. Prepare output file
    let now = Utc::now();
    let filename = format!("padz-{}.tar.gz", now.format("%Y-%m-%d_%H:%M:%S"));
    let file = File::create(&filename).map_err(PadzError::Io)?;

    // 3. Write archive
    write_archive(file, &pads)?;

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!("Exported to {}", filename)));
    Ok(result)
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
        pads_by_selectors(store, scope, selectors, false)
    }
}

fn write_archive<W: Write>(writer: W, pads: &[DisplayPad]) -> Result<()> {
    let enc = GzEncoder::new(writer, Compression::default());
    let mut tar = tar::Builder::new(enc);

    for dp in pads {
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
) -> Result<CmdResult> {
    let pads = resolve_pads(store, scope, selectors)?;

    if pads.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to export."));
        return Ok(res);
    }

    let format = SingleFileFormat::from_filename(title);
    let result = merge_pads_to_single_file(&pads, title, format);

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
    pads: &[DisplayPad],
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
fn merge_as_text(pads: &[DisplayPad]) -> String {
    let mut output = String::new();

    for (i, dp) in pads.iter().enumerate() {
        if i > 0 {
            output.push_str("\n\n");
        }

        // Add header with pad title
        let title = &dp.pad.metadata.title;
        let separator = "=".repeat(title.len().max(40));
        output.push_str(&separator);
        output.push('\n');
        output.push_str(title);
        output.push('\n');
        output.push_str(&separator);
        output.push_str("\n\n");

        // Add pad content (skip the title line since we already printed it)
        let content = &dp.pad.content;
        if let Some(body_start) = content.find("\n\n") {
            output.push_str(content[body_start + 2..].trim());
        }
    }

    output
}

/// Merge pads as markdown with the export title as H1 and bumped headers.
fn merge_as_markdown(pads: &[DisplayPad], export_title: &str) -> String {
    let mut output = String::new();

    // Export title as H1
    output.push_str("# ");
    output.push_str(export_title);
    output.push_str("\n\n");

    for (i, dp) in pads.iter().enumerate() {
        if i > 0 {
            output.push_str("\n\n---\n\n");
        }

        // Pad title becomes H2
        output.push_str("## ");
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
            // Bump all headers in the body
            let bumped = bump_markdown_headers(body);
            output.push_str(&bumped);
        }
    }

    output
}

/// Bump all markdown header levels by 2 (H1->H3, H2->H4, etc., H6 stays H6).
/// Uses pulldown-cmark for proper markdown parsing.
pub fn bump_markdown_headers(content: &str) -> String {
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
                let new_level = bump_heading_level(level);
                Event::Start(Tag::Heading {
                    level: new_level,
                    id,
                    classes,
                    attrs,
                })
            }
            Event::End(TagEnd::Heading(level)) => {
                let new_level = bump_heading_level(level);
                Event::End(TagEnd::Heading(new_level))
            }
            other => other,
        })
        .collect();

    let mut output = String::new();
    // cmark returns Result, unwrap is safe for valid events
    cmark(events.iter(), &mut output).expect("cmark serialization failed");
    output
}

/// Bump a heading level by 2, capped at H6.
fn bump_heading_level(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 => HeadingLevel::H3,
        HeadingLevel::H2 => HeadingLevel::H4,
        HeadingLevel::H3 => HeadingLevel::H5,
        HeadingLevel::H4 => HeadingLevel::H6,
        HeadingLevel::H5 => HeadingLevel::H6,
        HeadingLevel::H6 => HeadingLevel::H6,
    }
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
        create::run(
            &mut store,
            Scope::Project,
            "Active".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        let del_pad = crate::model::Pad::new("Deleted".into(), "".into());
        store
            .save_pad(&del_pad, Scope::Project, crate::store::Bucket::Deleted)
            .unwrap();

        let pads = resolve_pads(&store, Scope::Project, &[]).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].pad.metadata.title, "Active");
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
            Vec::new(),
        )
        .unwrap();
        let pads = resolve_pads(&store, Scope::Project, &[]).unwrap();

        let mut buf = Vec::new();
        write_archive(&mut buf, &pads).unwrap();

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

        let pad1 = DisplayPad {
            pad: crate::model::Pad::new("First Pad".into(), "Content one".into()),
            index: DisplayIndex::Regular(1),
            matches: None,
            children: vec![],
        };
        let pad2 = DisplayPad {
            pad: crate::model::Pad::new("Second Pad".into(), "Content two".into()),
            index: DisplayIndex::Regular(2),
            matches: None,
            children: vec![],
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

        let pad1 = DisplayPad {
            pad: crate::model::Pad::new("First Pad".into(), "# Internal H1\n\nBody text".into()),
            index: DisplayIndex::Regular(1),
            matches: None,
            children: vec![],
        };
        let pad2 = DisplayPad {
            pad: crate::model::Pad::new("Second Pad".into(), "## Internal H2\n\nMore body".into()),
            index: DisplayIndex::Regular(2),
            matches: None,
            children: vec![],
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
        let res = run(&store, Scope::Project, &[]).unwrap();
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("No pads to export")));

        let res_single = run_single_file(&store, Scope::Project, &[], "out.md").unwrap();
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
        create::run(
            &mut store,
            Scope::Project,
            "A".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

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

        let res = run_single_file(&store, Scope::Project, &[], &input_title).unwrap();

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
}
