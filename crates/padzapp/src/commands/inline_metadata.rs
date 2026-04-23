//! Inline metadata for md/lex files.
//!
//! Two dialects, both carrying the same key set as the JSON archive's
//! `metadata` object:
//!
//! ## Markdown — YAML frontmatter (dotted keys under `padz.`)
//!
//! ```markdown
//! ---
//! padz.schema_version: 1
//! padz.id: "aaaa-..."
//! padz.created_at: "2026-04-22T00:00:00Z"
//! padz.status: Planned
//! padz.tags:
//!   - work
//! ---
//!
//! Title
//!
//! body...
//! ```
//!
//! ## Lex — top-of-document annotations
//!
//! ```lex
//! :: padz.schema_version :: 1
//! :: padz.id :: aaaa-...
//! :: padz.status :: Planned
//! :: padz.tags :: work,personal
//!
//! Title
//!
//!     body
//! ```
//!
//! ## Import detection
//!
//! - md: starts with a `---` line (YAML frontmatter fence) → parse until
//!   the closing `---`
//! - lex: starts with one or more `:: padz.KEY :: VALUE` lines → parse
//!   consecutive leading annotations
//!
//! Files without the opening sentinel are imported as plain content (no
//! metadata), preserving backwards compatibility.
//!
//! ## Non-goals
//!
//! We serialize **as text**, not through a YAML library. The key set is
//! fixed and small, so the hand-rolled output is easier to audit than a
//! dependency. Parsing uses `serde_yaml` for md (since arbitrary user
//! frontmatter can be complex) and a line-by-line parser for lex.

use crate::commands::metadata_schema::SCHEMA_VERSION;
use crate::model::{Metadata, TodoStatus};
use crate::store::Bucket;
use chrono::SecondsFormat;
use serde_json::{Map, Value};

pub const PADZ_PREFIX: &str = "padz.";

/// Serialize a pad's metadata as YAML frontmatter. Returns the full
/// `---\n...\n---\n\n` block, ready to prepend to the pad content.
pub fn serialize_md_frontmatter(meta: &Metadata, bucket: Bucket) -> String {
    let mut out = String::from("---\n");
    out.push_str(&format!("padz.schema_version: {}\n", SCHEMA_VERSION));
    out.push_str(&format!("padz.id: \"{}\"\n", meta.id));
    out.push_str(&format!(
        "padz.created_at: \"{}\"\n",
        meta.created_at.to_rfc3339_opts(SecondsFormat::Secs, true)
    ));
    out.push_str(&format!(
        "padz.updated_at: \"{}\"\n",
        meta.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true)
    ));
    out.push_str(&format!("padz.is_pinned: {}\n", meta.is_pinned));
    match meta.pinned_at {
        Some(ts) => out.push_str(&format!(
            "padz.pinned_at: \"{}\"\n",
            ts.to_rfc3339_opts(SecondsFormat::Secs, true)
        )),
        None => out.push_str("padz.pinned_at: null\n"),
    }
    out.push_str(&format!(
        "padz.delete_protected: {}\n",
        meta.delete_protected
    ));
    match meta.parent_id {
        Some(p) => out.push_str(&format!("padz.parent_id: \"{}\"\n", p)),
        None => out.push_str("padz.parent_id: null\n"),
    }
    out.push_str(&format!(
        "padz.status: {}\n",
        todo_status_label(meta.status)
    ));
    if meta.tags.is_empty() {
        out.push_str("padz.tags: []\n");
    } else {
        out.push_str("padz.tags:\n");
        for t in &meta.tags {
            out.push_str(&format!("  - {}\n", yaml_quote(t)));
        }
    }
    out.push_str(&format!("padz.bucket: {}\n", bucket_label(bucket)));
    out.push_str("---\n\n");
    out
}

/// Serialize a pad's metadata as lex annotations, followed by a blank line
/// so the document body stays valid lex.
pub fn serialize_lex_metadata(meta: &Metadata, bucket: Bucket) -> String {
    let mut out = String::new();
    out.push_str(&format!(":: padz.schema_version :: {}\n", SCHEMA_VERSION));
    out.push_str(&format!(":: padz.id :: {}\n", meta.id));
    out.push_str(&format!(
        ":: padz.created_at :: {}\n",
        meta.created_at.to_rfc3339_opts(SecondsFormat::Secs, true)
    ));
    out.push_str(&format!(
        ":: padz.updated_at :: {}\n",
        meta.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true)
    ));
    out.push_str(&format!(":: padz.is_pinned :: {}\n", meta.is_pinned));
    match meta.pinned_at {
        Some(ts) => out.push_str(&format!(
            ":: padz.pinned_at :: {}\n",
            ts.to_rfc3339_opts(SecondsFormat::Secs, true)
        )),
        None => out.push_str(":: padz.pinned_at :: null\n"),
    }
    out.push_str(&format!(
        ":: padz.delete_protected :: {}\n",
        meta.delete_protected
    ));
    match meta.parent_id {
        Some(p) => out.push_str(&format!(":: padz.parent_id :: {}\n", p)),
        None => out.push_str(":: padz.parent_id :: null\n"),
    }
    out.push_str(&format!(
        ":: padz.status :: {}\n",
        todo_status_label(meta.status)
    ));
    // Comma-separated for readability; parser tolerates surrounding spaces.
    out.push_str(&format!(":: padz.tags :: {}\n", meta.tags.join(",")));
    out.push_str(&format!(":: padz.bucket :: {}\n", bucket_label(bucket)));
    out.push('\n');
    out
}

/// Extract metadata + body from an md file. Returns `(metadata_json, body)`
/// when YAML frontmatter is detected, or `None` when it isn't (caller treats
/// as plain content).
///
/// Body has the frontmatter fence and its following blank line removed.
pub fn parse_md_frontmatter(raw: &str) -> Option<(Value, String)> {
    let stripped = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    if !stripped.starts_with("---") {
        return None;
    }
    // First line must be exactly "---" (possibly with trailing whitespace)
    let mut lines = stripped.split_inclusive('\n');
    let first = lines.next()?;
    if first.trim_end_matches('\n').trim() != "---" {
        return None;
    }

    let mut yaml_buf = String::new();
    let mut end_found = false;
    let mut consumed = first.len();
    for line in lines {
        consumed += line.len();
        let trimmed = line.trim_end_matches('\n');
        if trimmed.trim() == "---" {
            end_found = true;
            break;
        }
        yaml_buf.push_str(line);
    }
    if !end_found {
        return None;
    }

    let body: String = stripped[consumed..].trim_start_matches('\n').to_string();

    let mut metadata = Map::new();
    for (raw_key, raw_val) in parse_yaml_frontmatter(&yaml_buf) {
        if let Some(bare) = raw_key.strip_prefix(PADZ_PREFIX) {
            metadata.insert(bare.to_string(), raw_val);
        }
    }
    if metadata.is_empty() {
        return None;
    }

    Some((Value::Object(metadata), body))
}

/// Extract metadata + body from a lex file. Recognizes leading
/// `:: padz.KEY :: VALUE` annotations; stops at the first non-annotation
/// line (blank or otherwise).
pub fn parse_lex_metadata(raw: &str) -> Option<(Value, String)> {
    let stripped = raw.strip_prefix('\u{feff}').unwrap_or(raw);
    if !stripped.starts_with(":: padz.") {
        return None;
    }

    let mut metadata = Map::new();
    let mut consumed = 0usize;
    for line in stripped.split_inclusive('\n') {
        // Only strip the trailing newline — preserve trailing spaces so a
        // `:: key :: ` annotation with an empty value still parses.
        let no_newline = line.trim_end_matches('\n');
        if !no_newline.starts_with(":: padz.") {
            break;
        }
        let Some((key, value)) = parse_lex_annotation(no_newline) else {
            break;
        };
        if let Some(bare) = key.strip_prefix(PADZ_PREFIX) {
            metadata.insert(bare.to_string(), coerce_scalar(bare, value.trim()));
        }
        consumed += line.len();
    }
    if metadata.is_empty() {
        return None;
    }

    // Skip the single blank line that separates metadata from the document body.
    let body = stripped[consumed..].trim_start_matches('\n').to_string();

    Some((Value::Object(metadata), body))
}

/// Parse `":: KEY :: VALUE"` into `(KEY, VALUE)`. Returns None on malformed.
fn parse_lex_annotation(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix(":: ")?;
    // Find the middle `::` separator after the key.
    let mid = rest.find(" :: ")?;
    let key = &rest[..mid];
    let value = &rest[mid + 4..]; // skip " :: "
                                  // Drop a trailing `::` if present (block-style annotation shorthand)
    let value = value.trim_end_matches(':').trim_end();
    Some((key, value))
}

/// Coerce a string lex/md scalar into the appropriate JSON value.
///
/// Known typed keys get typed values; everything else stays a string so that
/// `metadata_apply` can decide what to do with it.
fn coerce_scalar(key: &str, raw: &str) -> Value {
    match key {
        "schema_version" => raw
            .parse::<u64>()
            .map(|n| Value::Number(n.into()))
            .unwrap_or_else(|_| Value::String(raw.to_string())),
        "is_pinned" | "delete_protected" => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::String(raw.to_string()),
        },
        "pinned_at" | "parent_id" => {
            if raw == "null" || raw.is_empty() {
                Value::Null
            } else {
                Value::String(raw.to_string())
            }
        }
        "tags" => {
            // Comma-separated list. Empty string → empty array.
            if raw.is_empty() {
                Value::Array(Vec::new())
            } else {
                Value::Array(
                    raw.split(',')
                        .map(|t| Value::String(t.trim().to_string()))
                        .filter(|v| v.as_str().is_some_and(|s| !s.is_empty()))
                        .collect(),
                )
            }
        }
        _ => Value::String(raw.to_string()),
    }
}

/// Hand-rolled YAML frontmatter parser for the subset we emit + tolerate.
///
/// Handles:
/// - `key: value` scalars (unquoted or `"…"`-quoted strings)
/// - `key: null`, `key: true/false`, `key: 123`
/// - `key: []` (empty array)
/// - `key:\n  - item\n  - item` (list of strings)
///
/// Returns a flat list of (key, Value). Keys outside our `padz.` namespace
/// are preserved verbatim — the caller filters them out.
fn parse_yaml_frontmatter(yaml: &str) -> Vec<(String, Value)> {
    let mut out: Vec<(String, Value)> = Vec::new();
    let lines: Vec<&str> = yaml.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_end();
        if trimmed.trim().is_empty() || trimmed.trim_start().starts_with('#') {
            i += 1;
            continue;
        }
        // Only top-level keys (no leading whitespace) are treated as padz metadata
        if line.starts_with(' ') || line.starts_with('\t') {
            i += 1;
            continue;
        }
        let Some((key, rest)) = line.split_once(':') else {
            i += 1;
            continue;
        };
        let key = key.trim().to_string();
        let rest = rest.trim();
        if rest.is_empty() {
            // Possibly a list follows
            let mut items = Vec::new();
            let mut j = i + 1;
            while j < lines.len() {
                let l = lines[j];
                let t = l.trim_start();
                if !(l.starts_with(' ') || l.starts_with('\t')) {
                    break;
                }
                if let Some(item) = t.strip_prefix("- ") {
                    items.push(Value::String(yaml_unquote(item).to_string()));
                } else if t.is_empty() {
                    // blank line — stop
                    break;
                } else {
                    break;
                }
                j += 1;
            }
            out.push((key, Value::Array(items)));
            i = j;
        } else {
            out.push((key, parse_yaml_scalar(rest)));
            i += 1;
        }
    }
    out
}

fn parse_yaml_scalar(raw: &str) -> Value {
    if raw == "null" || raw == "~" {
        return Value::Null;
    }
    if raw == "true" {
        return Value::Bool(true);
    }
    if raw == "false" {
        return Value::Bool(false);
    }
    if raw == "[]" {
        return Value::Array(Vec::new());
    }
    if let Some(inner) = raw.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        return Value::String(inner.to_string());
    }
    if let Ok(n) = raw.parse::<i64>() {
        return Value::Number(n.into());
    }
    Value::String(raw.to_string())
}

fn yaml_unquote(s: &str) -> &str {
    s.strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .unwrap_or(s)
}

/// Quote a tag value only if it contains characters that would break YAML.
fn yaml_quote(s: &str) -> String {
    if s.chars()
        .any(|c| !(c.is_alphanumeric() || c == '-' || c == '_'))
    {
        format!("\"{}\"", s.replace('"', "\\\""))
    } else {
        s.to_string()
    }
}

fn todo_status_label(s: TodoStatus) -> &'static str {
    match s {
        TodoStatus::Planned => "Planned",
        TodoStatus::InProgress => "InProgress",
        TodoStatus::Done => "Done",
    }
}

fn bucket_label(b: Bucket) -> &'static str {
    match b {
        Bucket::Active => "Active",
        Bucket::Archived => "Archived",
        Bucket::Deleted => "Deleted",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn sample_meta() -> Metadata {
        let id = Uuid::parse_str("11111111-2222-3333-4444-555555555555").unwrap();
        let mut m = Metadata::new("Example Title".into());
        m.id = id;
        m.created_at = Utc.with_ymd_and_hms(2026, 4, 22, 10, 30, 0).unwrap();
        m.updated_at = Utc.with_ymd_and_hms(2026, 4, 22, 11, 0, 0).unwrap();
        m.is_pinned = true;
        m.pinned_at = Some(Utc.with_ymd_and_hms(2026, 4, 22, 11, 5, 0).unwrap());
        m.delete_protected = true;
        m.status = TodoStatus::Done;
        m.tags = vec!["work".into(), "rust".into()];
        m
    }

    #[test]
    fn test_serialize_md_frontmatter_roundtrip() {
        let meta = sample_meta();
        let block = serialize_md_frontmatter(&meta, Bucket::Active);
        assert!(block.starts_with("---\n"));
        assert!(block.ends_with("---\n\n"));

        let full = format!("{}Example Title\n\nBody text", block);
        let (parsed, body) = parse_md_frontmatter(&full).expect("frontmatter should parse");

        assert_eq!(body, "Example Title\n\nBody text");
        assert_eq!(parsed["id"], Value::String(meta.id.to_string()));
        assert_eq!(parsed["is_pinned"], Value::Bool(true));
        assert_eq!(parsed["status"], Value::String("Done".into()));
        assert_eq!(
            parsed["tags"],
            Value::Array(vec![
                Value::String("work".into()),
                Value::String("rust".into()),
            ])
        );
    }

    #[test]
    fn test_parse_md_frontmatter_no_fence_returns_none() {
        let raw = "No frontmatter here\n\nBody";
        assert!(parse_md_frontmatter(raw).is_none());
    }

    #[test]
    fn test_parse_md_frontmatter_unterminated_returns_none() {
        let raw = "---\npadz.id: \"abc\"\nno closing fence";
        assert!(parse_md_frontmatter(raw).is_none());
    }

    #[test]
    fn test_parse_md_frontmatter_ignores_non_padz_keys() {
        let raw = "---\nauthor: Alice\npadz.status: Done\n---\n\nTitle\n\nBody";
        let (parsed, body) = parse_md_frontmatter(raw).unwrap();
        assert_eq!(body, "Title\n\nBody");
        assert_eq!(parsed["status"], Value::String("Done".into()));
        assert!(parsed.get("author").is_none(), "non-padz keys stripped");
    }

    #[test]
    fn test_serialize_lex_metadata_roundtrip() {
        let meta = sample_meta();
        let block = serialize_lex_metadata(&meta, Bucket::Active);
        let full = format!("{}Example Title\n\n    Body", block);

        let (parsed, body) = parse_lex_metadata(&full).expect("lex metadata should parse");
        assert_eq!(body, "Example Title\n\n    Body");
        assert_eq!(parsed["id"], Value::String(meta.id.to_string()));
        assert_eq!(parsed["status"], Value::String("Done".into()));
        assert_eq!(parsed["is_pinned"], Value::Bool(true));
        assert_eq!(
            parsed["tags"],
            Value::Array(vec![
                Value::String("work".into()),
                Value::String("rust".into()),
            ])
        );
    }

    #[test]
    fn test_parse_lex_metadata_no_prefix_returns_none() {
        let raw = "Regular lex doc\n\n    body\n";
        assert!(parse_lex_metadata(raw).is_none());
    }

    #[test]
    fn test_parse_lex_metadata_empty_tags() {
        let raw = ":: padz.id :: abc\n:: padz.tags :: \n\nTitle\n";
        let (parsed, _) = parse_lex_metadata(raw).unwrap();
        assert_eq!(parsed["tags"], Value::Array(vec![]));
    }
}
