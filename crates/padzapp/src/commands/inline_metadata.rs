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
//! ## Implementation
//!
//! Markdown frontmatter uses `serde_yaml` for both emit and parse — the YAML
//! surface is full-featured enough that hand-rolling is a foot-gun. Lex
//! annotations are hand-parsed because the syntax is custom to lex and no
//! published parser is (yet) a dependency here.

use crate::commands::metadata_schema::SCHEMA_VERSION;
use crate::model::{Metadata, TodoStatus};
use crate::store::Bucket;
use chrono::SecondsFormat;
use serde_json::{Map, Value};

pub const PADZ_PREFIX: &str = "padz.";

/// Serialize a pad's metadata as YAML frontmatter. Returns the full
/// `---\n...\n---\n\n` block, ready to prepend to the pad content.
pub fn serialize_md_frontmatter(meta: &Metadata, bucket: Bucket) -> String {
    let mapping = metadata_to_yaml_mapping(meta, bucket);
    let body = serde_yaml::to_string(&mapping)
        .expect("serializing fixed-schema metadata to YAML cannot fail");
    format!("---\n{}---\n\n", body)
}

fn metadata_to_yaml_mapping(meta: &Metadata, bucket: Bucket) -> serde_yaml::Mapping {
    use serde_yaml::Value as Y;
    let mut m = serde_yaml::Mapping::new();
    let k = |name: &str| Y::String(format!("{}{}", PADZ_PREFIX, name));
    m.insert(k("schema_version"), Y::Number(SCHEMA_VERSION.into()));
    m.insert(k("id"), Y::String(meta.id.to_string()));
    m.insert(
        k("created_at"),
        Y::String(meta.created_at.to_rfc3339_opts(SecondsFormat::Secs, true)),
    );
    m.insert(
        k("updated_at"),
        Y::String(meta.updated_at.to_rfc3339_opts(SecondsFormat::Secs, true)),
    );
    m.insert(k("is_pinned"), Y::Bool(meta.is_pinned));
    m.insert(
        k("pinned_at"),
        match meta.pinned_at {
            Some(ts) => Y::String(ts.to_rfc3339_opts(SecondsFormat::Secs, true)),
            None => Y::Null,
        },
    );
    m.insert(k("delete_protected"), Y::Bool(meta.delete_protected));
    m.insert(
        k("parent_id"),
        match meta.parent_id {
            Some(p) => Y::String(p.to_string()),
            None => Y::Null,
        },
    );
    m.insert(
        k("status"),
        Y::String(todo_status_label(meta.status).into()),
    );
    m.insert(
        k("tags"),
        Y::Sequence(meta.tags.iter().cloned().map(Y::String).collect()),
    );
    m.insert(k("bucket"), Y::String(bucket_label(bucket).into()));
    m
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

    // Malformed YAML: treat the whole document as plain content.
    let yaml_value: serde_yaml::Value = serde_yaml::from_str(&yaml_buf).ok()?;
    let yaml_map = match yaml_value {
        serde_yaml::Value::Mapping(m) => m,
        _ => return None,
    };

    let mut metadata = Map::new();
    for (key, value) in yaml_map {
        let key_str = match key {
            serde_yaml::Value::String(s) => s,
            _ => continue,
        };
        if let Some(bare) = key_str.strip_prefix(PADZ_PREFIX) {
            metadata.insert(bare.to_string(), yaml_to_json(value));
        }
    }
    if metadata.is_empty() {
        return None;
    }

    Some((Value::Object(metadata), body))
}

/// Convert a `serde_yaml::Value` into a `serde_json::Value`.
///
/// Non-string map keys are dropped (our schema never uses them); YAML tags are
/// stripped and the inner value is used.
fn yaml_to_json(v: serde_yaml::Value) -> Value {
    match v {
        serde_yaml::Value::Null => Value::Null,
        serde_yaml::Value::Bool(b) => Value::Bool(b),
        serde_yaml::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Number(i.into())
            } else if let Some(u) = n.as_u64() {
                Value::Number(u.into())
            } else if let Some(f) = n.as_f64() {
                serde_json::Number::from_f64(f)
                    .map(Value::Number)
                    .unwrap_or(Value::Null)
            } else {
                Value::Null
            }
        }
        serde_yaml::Value::String(s) => Value::String(s),
        serde_yaml::Value::Sequence(seq) => {
            Value::Array(seq.into_iter().map(yaml_to_json).collect())
        }
        serde_yaml::Value::Mapping(map) => {
            let mut out = Map::new();
            for (k, v) in map {
                if let serde_yaml::Value::String(key) = k {
                    out.insert(key, yaml_to_json(v));
                }
            }
            Value::Object(out)
        }
        serde_yaml::Value::Tagged(tagged) => yaml_to_json(tagged.value),
    }
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
