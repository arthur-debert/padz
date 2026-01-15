//! # Domain Model: Pad Content Format and Normalization
//!
//! This module defines the core data structures for padz: [`Pad`], [`Metadata`], and [`Scope`].
//! It also handles content normalization, which is crucial for data integrity.
//!
//! ## The Problem
//!
//! Users dump text into notes in chaotic formats:
//! - Sometimes they write a title, sometimes not.
//! - Sometimes they leave blank lines at the top.
//! - Sometimes they pipe in logs or code snippets.
//!
//! If we display this raw content in lists/peeks, the UI looks broken.
//! We need a "Canonical Format" without forcing the user to fill out a form.
//!
//! ## The Canonical Format
//!
//! ```text
//! Title Line     <-- Line 1 (first non-empty line)
//!                <-- Line 2 (blank separator)
//! Body Content   <-- Line 3+ (remaining content, trimmed)
//! ```
//!
//! ## Normalization Pipeline
//!
//! Padz accepts any UTF-8 text but normalizes it upon save:
//!
//! 1. **Title Extraction**: Trim input, take first non-empty line as title.
//! 2. **Body Extraction**: Take remaining lines, trim whitespace.
//! 3. **Reassembly**:
//!    - If body is non-empty: `"{title}\n\n{body}"`
//!    - If body is empty: Just the title (no trailing newlines)
//!
//! ## Title Truncation
//!
//! - **In File**: The full title is stored as the first line.
//! - **In Metadata**: Truncated to 60 characters for display (59 chars + ellipsis `…`).
//!
//! ## Edge Cases
//!
//! - **One-Liner**: `"Only Title"` → Normalized to `"Only Title"` (no separator or body).
//! - **Empty Input**: Rejected. The store garbage collects empty files.
//! - **Multiple Blank Lines**: Collapsed to a single separator line.
//! - **Leading Blank Lines**: Stripped before title extraction.
//!
//! ## Key Functions
//!
//! - [`normalize_pad_content`]: Normalizes title and body into canonical format
//! - [`extract_title_and_body`]: Parses raw content into (title, body) tuple
//! - [`parse_pad_content`]: Combines extraction and normalization

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    Project,
    Global,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TodoStatus {
    Planned,
    InProgress,
    Done,
}

impl Default for TodoStatus {
    fn default() -> Self {
        Self::Planned
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Metadata {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_pinned: bool,
    pub pinned_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    pub delete_protected: bool,
    pub parent_id: Option<Uuid>,
    // We store the title in metadata to list without reading content files
    pub title: String,
    #[serde(default)]
    pub status: TodoStatus,
    /// Tags assigned to this pad (references tag names from the tag registry)
    #[serde(default)]
    pub tags: Vec<String>,
}

// Custom deserializer to handle legacy data where `delete_protected` is missing.
// If missing, it defaults to the value of `is_pinned`.
impl<'de> Deserialize<'de> for Metadata {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = MetadataHelper::deserialize(deserializer)?;

        Ok(Metadata {
            id: helper.id,
            created_at: helper.created_at,
            updated_at: helper.updated_at,
            is_pinned: helper.is_pinned,
            pinned_at: helper.pinned_at,
            is_deleted: helper.is_deleted,
            deleted_at: helper.deleted_at,
            // If delete_protected is missing (None), default to is_pinned.
            // This ensures legacy pinned pads are protected.
            delete_protected: helper.delete_protected.unwrap_or(helper.is_pinned),
            parent_id: helper.parent_id,
            title: helper.title,
            status: helper.status.unwrap_or(TodoStatus::Planned),
            tags: helper.tags,
        })
    }
}

#[derive(Deserialize)]
struct MetadataHelper {
    id: Uuid,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    is_pinned: bool,
    pinned_at: Option<DateTime<Utc>>,
    is_deleted: bool,
    deleted_at: Option<DateTime<Utc>>,
    #[serde(default)]
    delete_protected: Option<bool>,
    #[serde(default)]
    parent_id: Option<Uuid>,
    title: String,
    #[serde(default)]
    status: Option<TodoStatus>,
    #[serde(default)]
    tags: Vec<String>,
}

impl Metadata {
    pub fn new(title: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            created_at: now,
            updated_at: now,
            is_pinned: false,
            pinned_at: None,
            is_deleted: false,
            deleted_at: None,
            delete_protected: false,
            parent_id: None,
            title,
            status: TodoStatus::Planned,
            tags: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pad {
    pub metadata: Metadata,
    pub content: String,
}

impl Pad {
    pub fn new(title: String, content: String) -> Self {
        let (normalized_title, normalized_content) = normalize_pad_content(&title, &content);
        Self {
            metadata: Metadata::new(normalized_title),
            content: normalized_content,
        }
    }

    /// Updates the pad content from a raw string, handling title extraction and normalization.
    pub fn update_from_raw(&mut self, raw: &str) {
        if let Some((title, content)) = parse_pad_content(raw) {
            self.metadata.title = title;
            self.content = content;
            self.metadata.updated_at = Utc::now();
        }
    }
}

/// Normalizes pad content components into a canonical format.
/// Returns (truncated_title, canonical_full_text).
pub fn normalize_pad_content(title: &str, body: &str) -> (String, String) {
    let clean_title = title.trim();
    // Title is first line of content, 60 chars truncated for display (metadata)
    // BUT we store the full title in the file content.
    // The requirement says "title is defined as the first line... which is 60 chars truncated for display".
    // It also says "The canonical representation is to store: <title>\n\n<content>".
    // So for metadata we truncate, for content we keep full line?
    // "Title ... reduced to 60 chars truncated for display" - implies metadata storage.
    // Let's truncate metadata title but keep full title in content.

    let display_title = if clean_title.chars().count() > 60 {
        let truncated: String = clean_title.chars().take(59).collect();
        format!("{}…", truncated)
    } else {
        clean_title.to_string()
    };

    // Body normalization:
    // 1. Strip leading/trailing whitespaces (including newlines) from the raw body text
    // 2. We will insert exactly one blank line between title and body in the final output
    let clean_body = body.trim();

    let full_content = if clean_body.is_empty() {
        // "One line pads has a title but no content"
        // Title\n\n (empty) -> just Title\n ?
        // Requirement: "1. <title> 2. <blank line> 3. <content>"
        // "That means that a one line pads has a title but no content (legal)"
        // Example output shows: <some text> -> Title \n <blank line> -> only one separation line ...
        // If content is empty, maybe we don't need the blank line?
        // "File is blank line stripped."
        // Let's stick to "Title\n\nContent". If Content is empty, it's "Title\n\n".
        // But the example shows:
        // Text -> Title
        // Blank
        // Text
        // ...
        // If the bottom text is missing, do we need the blank?
        // "File is blank line stripped" - usually implies no trailing blanks.
        // If I have "Title\n\n", the last \n is trailing?
        // Let's assume for now: Title + \n\n + Body. If Body is empty -> Title.
        // Wait, "normalized to: <text>\n<blank>\n<text>".
        // If the second text is missing, it would be "<text>\n<blank>". which is trailing whitespace.
        // Let's assume if body is empty, we just store Title.
        clean_title.to_string()
    } else {
        format!("{}\n\n{}", clean_title, clean_body)
    };

    (display_title, full_content)
}

/// Parses raw file content into title and body components (not fully normalized yet).
/// Returns (Title, Body).
pub fn extract_title_and_body(raw: &str) -> Option<(String, String)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut lines = trimmed.lines();
    // First line is title
    let title = lines.next().unwrap_or("").trim().to_string();

    // Skip potential blank line separator if present, but the requirement says "Blank lines between title and content (collapsed to 1 line)".
    // We already trimmed the whole string, so we are at the first line.
    // The rest of the iterator is the body.
    // We need to re-join and trim the body.

    // Collect rest
    let rest_raw = lines.collect::<Vec<&str>>().join("\n");
    let body = rest_raw.trim().to_string();

    Some((title, body))
}

/// Parses raw file content into title and fully normalized content.
/// Returns None if the file has no text at all.
/// Returns (TruncatedTitle, NormalizedFullContent).
pub fn parse_pad_content(raw: &str) -> Option<(String, String)> {
    let (title, body) = extract_title_and_body(raw)?;
    Some(normalize_pad_content(&title, &body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_simple() {
        let (title, content) = normalize_pad_content("My Title", "My Content");
        assert_eq!(title, "My Title");
        assert_eq!(content, "My Title\n\nMy Content");
    }

    #[test]
    fn test_normalize_empty_body() {
        let (title, content) = normalize_pad_content("Just Title", "");
        assert_eq!(title, "Just Title");
        assert_eq!(content, "Just Title");
    }

    #[test]
    fn test_normalize_truncates_title_metadata() {
        let long_title = "a".repeat(100);
        let (title, content) = normalize_pad_content(&long_title, "Body");
        // Title should be 59 chars + ellipsis = 60 chars total
        assert_eq!(title.chars().count(), 60);
        assert!(
            title.ends_with('…'),
            "Truncated title should end with ellipsis"
        );
        assert_eq!(content, format!("{}\n\nBody", long_title));
    }

    #[test]
    fn test_parse_valid() {
        let raw = "Title\n\nBody";
        let (title, content) = parse_pad_content(raw).unwrap();
        assert_eq!(title, "Title");
        assert_eq!(content, "Title\n\nBody");
    }

    #[test]
    fn test_parse_extra_blanks() {
        let raw = "\n\nTitle\n\n\n\nBody\n\n";
        let (title, content) = parse_pad_content(raw).unwrap();
        assert_eq!(title, "Title");
        assert_eq!(content, "Title\n\nBody");
    }

    #[test]
    fn test_parse_empty_invalid() {
        assert!(parse_pad_content("   \n   ").is_none());
    }

    #[test]
    fn test_parse_one_line() {
        let (title, content) = parse_pad_content("OneLine").unwrap();
        assert_eq!(title, "OneLine");
        assert_eq!(content, "OneLine");
    }

    #[test]
    fn test_metadata_serialization_roundtrip() {
        let parent_id = Uuid::new_v4();
        let mut meta = Metadata::new("Child Pad".to_string());
        meta.parent_id = Some(parent_id);

        // Serialize
        let json = serde_json::to_string(&meta).unwrap();

        // Deserialize
        let loaded: Metadata = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.id, meta.id);
        assert_eq!(loaded.parent_id, Some(parent_id));
        assert_eq!(loaded.title, "Child Pad");
    }

    #[test]
    fn test_legacy_metadata_deserialization() {
        let id = Uuid::new_v4();
        // JSON without parent_id (legacy format)
        let json = format!(
            r#"{{
            "id": "{}",
            "created_at": "2023-01-01T00:00:00Z",
            "updated_at": "2023-01-01T00:00:00Z",
            "is_pinned": false,
            "pinned_at": null,
            "is_deleted": false,
            "deleted_at": null,
            "delete_protected": false,
            "title": "Legacy Pad"
        }}"#,
            id
        );

        let loaded: Metadata = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.id, id);
        assert_eq!(loaded.parent_id, None);
        assert_eq!(loaded.title, "Legacy Pad");
    }

    #[test]
    fn test_metadata_deserialization_with_explicit_delete_protected() {
        let id = Uuid::new_v4();
        // JSON with explicit delete_protected = true, but is_pinned = false
        // This verifies we don't blindly copy is_pinned
        let json = format!(
            r#"{{
            "id": "{}",
            "created_at": "2023-01-01T00:00:00Z",
            "updated_at": "2023-01-01T00:00:00Z",
            "is_pinned": false,
            "pinned_at": null,
            "is_deleted": false,
            "deleted_at": null,
            "delete_protected": true,
            "title": "Protected Pad"
        }}"#,
            id
        );

        let loaded: Metadata = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.id, id);
        assert!(loaded.delete_protected);
        assert!(!loaded.is_pinned);
    }

    #[test]
    fn test_update_from_raw() {
        let mut pad = Pad::new("Old Title".to_string(), "Old Content".to_string());
        let old_updated_at = pad.metadata.updated_at;

        // Sleep briefly to ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        pad.update_from_raw("New Title\n\nNew Content");

        assert_eq!(pad.metadata.title, "New Title");
        assert_eq!(pad.content, "New Title\n\nNew Content");
        assert!(pad.metadata.updated_at > old_updated_at);
    }

    #[test]
    fn test_update_from_raw_ignores_empty() {
        let mut pad = Pad::new("Old Title".to_string(), "Old Content".to_string());
        let old_updated_at = pad.metadata.updated_at;
        let old_content = pad.content.clone();

        pad.update_from_raw("   ");

        assert_eq!(pad.content, old_content);
        assert_eq!(pad.metadata.updated_at, old_updated_at);
    }

    #[test]
    fn test_legacy_metadata_without_tags() {
        let id = Uuid::new_v4();
        // JSON without tags field (legacy format before tags were added)
        let json = format!(
            r#"{{
            "id": "{}",
            "created_at": "2023-01-01T00:00:00Z",
            "updated_at": "2023-01-01T00:00:00Z",
            "is_pinned": false,
            "pinned_at": null,
            "is_deleted": false,
            "deleted_at": null,
            "delete_protected": false,
            "title": "Legacy Pad Without Tags"
        }}"#,
            id
        );

        let loaded: Metadata = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.id, id);
        assert_eq!(loaded.title, "Legacy Pad Without Tags");
        // Tags should default to empty vector
        assert!(loaded.tags.is_empty());
    }

    #[test]
    fn test_metadata_with_tags_roundtrip() {
        let mut meta = Metadata::new("Tagged Pad".to_string());
        meta.tags = vec!["work".to_string(), "rust".to_string()];

        // Serialize
        let json = serde_json::to_string(&meta).unwrap();

        // Deserialize
        let loaded: Metadata = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.id, meta.id);
        assert_eq!(loaded.title, "Tagged Pad");
        assert_eq!(loaded.tags, vec!["work", "rust"]);
    }

    #[test]
    fn test_new_metadata_has_empty_tags() {
        let meta = Metadata::new("New Pad".to_string());
        assert!(meta.tags.is_empty());
    }
}
