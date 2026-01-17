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

use crate::attributes::{AttrSideEffect, AttrValue};

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

    /// Get an attribute value by name.
    ///
    /// Returns `None` if the attribute name is not recognized.
    /// For known attributes, returns the current value wrapped in [`AttrValue`].
    ///
    /// # Supported Attributes
    ///
    /// | Name | Type | Description |
    /// |------|------|-------------|
    /// | `"pinned"` | `BoolWithTimestamp` | Pin state and when it was set |
    /// | `"deleted"` | `BoolWithTimestamp` | Deletion state and when deleted |
    /// | `"protected"` | `Bool` | Delete protection flag |
    /// | `"status"` | `Enum` | Todo status (Planned/InProgress/Done) |
    /// | `"tags"` | `List` | Assigned tag names |
    /// | `"parent"` | `Ref` | Parent pad UUID |
    ///
    /// # Example
    ///
    /// ```ignore
    /// let meta = Metadata::new("Test".into());
    /// assert_eq!(meta.get_attr("pinned").unwrap().as_bool(), Some(false));
    /// ```
    pub fn get_attr(&self, name: &str) -> Option<AttrValue> {
        match name {
            "pinned" => Some(AttrValue::BoolWithTimestamp {
                value: self.is_pinned,
                timestamp: self.pinned_at,
            }),
            "deleted" => Some(AttrValue::BoolWithTimestamp {
                value: self.is_deleted,
                timestamp: self.deleted_at,
            }),
            "protected" => Some(AttrValue::Bool(self.delete_protected)),
            "status" => Some(AttrValue::Enum(format!("{:?}", self.status))),
            "tags" => Some(AttrValue::List(self.tags.clone())),
            "parent" => Some(AttrValue::Ref(self.parent_id)),
            _ => None,
        }
    }

    /// Set an attribute value by name.
    ///
    /// Returns `None` if the attribute name is not recognized or the value type
    /// doesn't match. Returns `Some(AttrSideEffect)` indicating what action the
    /// caller should take after setting the attribute.
    ///
    /// # Coupled Attributes
    ///
    /// Some attributes have coupled behavior:
    /// - `"pinned"`: Also sets `delete_protected` to the same value
    ///
    /// # Side Effects
    ///
    /// The returned [`AttrSideEffect`] indicates what the caller should do:
    /// - `None`: No action needed
    /// - `PropagateStatusUp`: Call `propagate_status_change()` with `parent_id`
    /// - `ValidateTags(tags)`: Validate that the tags exist in the registry
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut meta = Metadata::new("Test".into());
    ///
    /// // Set pinned (also sets delete_protected)
    /// meta.set_attr("pinned", AttrValue::Bool(true));
    /// assert!(meta.is_pinned);
    /// assert!(meta.delete_protected);
    ///
    /// // Set status (returns PropagateStatusUp)
    /// let effect = meta.set_attr("status", AttrValue::Enum("Done".into()));
    /// assert_eq!(effect, Some(AttrSideEffect::PropagateStatusUp));
    /// ```
    pub fn set_attr(&mut self, name: &str, value: AttrValue) -> Option<AttrSideEffect> {
        match name {
            "pinned" => {
                let flag = value.as_bool()?;
                self.is_pinned = flag;
                self.pinned_at = if flag { Some(Utc::now()) } else { None };
                // Coupled: pinned also controls delete_protected
                self.delete_protected = flag;
                Some(AttrSideEffect::None)
            }
            "deleted" => {
                let flag = value.as_bool()?;
                self.is_deleted = flag;
                self.deleted_at = if flag { Some(Utc::now()) } else { None };
                // Note: deletion has its own side effect (status propagation)
                // but that's handled by the caller, not as a formal side effect here
                Some(AttrSideEffect::PropagateStatusUp)
            }
            "protected" => {
                let flag = value.as_bool()?;
                self.delete_protected = flag;
                Some(AttrSideEffect::None)
            }
            "status" => {
                let status_str = value.as_enum()?;
                self.status = match status_str {
                    "Planned" => TodoStatus::Planned,
                    "InProgress" => TodoStatus::InProgress,
                    "Done" => TodoStatus::Done,
                    _ => return None, // Invalid status value
                };
                Some(AttrSideEffect::PropagateStatusUp)
            }
            "tags" => {
                let tags = value.as_list()?.to_vec();
                let tags_for_validation = tags.clone();
                self.tags = tags;
                Some(AttrSideEffect::ValidateTags(tags_for_validation))
            }
            "parent" => {
                let parent_id = value.as_ref()?;
                self.parent_id = parent_id;
                // Changing parent triggers status propagation to both old and new parent
                Some(AttrSideEffect::PropagateStatusUp)
            }
            _ => None,
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

    // --- get_attr tests ---

    #[test]
    fn test_get_attr_pinned_default() {
        let meta = Metadata::new("Test".into());
        let value = meta.get_attr("pinned").unwrap();
        match value {
            crate::attributes::AttrValue::BoolWithTimestamp { value, timestamp } => {
                assert!(!value);
                assert!(timestamp.is_none());
            }
            _ => panic!("Expected BoolWithTimestamp"),
        }
    }

    #[test]
    fn test_get_attr_pinned_when_set() {
        let mut meta = Metadata::new("Test".into());
        meta.is_pinned = true;
        meta.pinned_at = Some(Utc::now());

        let value = meta.get_attr("pinned").unwrap();
        match value {
            crate::attributes::AttrValue::BoolWithTimestamp { value, timestamp } => {
                assert!(value);
                assert!(timestamp.is_some());
            }
            _ => panic!("Expected BoolWithTimestamp"),
        }
    }

    #[test]
    fn test_get_attr_deleted_default() {
        let meta = Metadata::new("Test".into());
        let value = meta.get_attr("deleted").unwrap();
        assert_eq!(value.as_bool(), Some(false));
    }

    #[test]
    fn test_get_attr_protected_default() {
        let meta = Metadata::new("Test".into());
        let value = meta.get_attr("protected").unwrap();
        assert_eq!(value.as_bool(), Some(false));
    }

    #[test]
    fn test_get_attr_protected_when_set() {
        let mut meta = Metadata::new("Test".into());
        meta.delete_protected = true;

        let value = meta.get_attr("protected").unwrap();
        assert_eq!(value.as_bool(), Some(true));
    }

    #[test]
    fn test_get_attr_status_default() {
        let meta = Metadata::new("Test".into());
        let value = meta.get_attr("status").unwrap();
        assert_eq!(value.as_enum(), Some("Planned"));
    }

    #[test]
    fn test_get_attr_status_variants() {
        let mut meta = Metadata::new("Test".into());

        meta.status = TodoStatus::InProgress;
        assert_eq!(
            meta.get_attr("status").unwrap().as_enum(),
            Some("InProgress")
        );

        meta.status = TodoStatus::Done;
        assert_eq!(meta.get_attr("status").unwrap().as_enum(), Some("Done"));
    }

    #[test]
    fn test_get_attr_tags_empty() {
        let meta = Metadata::new("Test".into());
        let value = meta.get_attr("tags").unwrap();
        assert_eq!(value.as_list(), Some(&[][..]));
    }

    #[test]
    fn test_get_attr_tags_with_values() {
        let mut meta = Metadata::new("Test".into());
        meta.tags = vec!["work".into(), "rust".into()];

        let value = meta.get_attr("tags").unwrap();
        let expected: Vec<String> = vec!["work".into(), "rust".into()];
        assert_eq!(value.as_list(), Some(expected.as_slice()));
    }

    #[test]
    fn test_get_attr_parent_none() {
        let meta = Metadata::new("Test".into());
        let value = meta.get_attr("parent").unwrap();
        assert_eq!(value.as_ref(), Some(None));
    }

    #[test]
    fn test_get_attr_parent_some() {
        let mut meta = Metadata::new("Test".into());
        let parent_id = Uuid::new_v4();
        meta.parent_id = Some(parent_id);

        let value = meta.get_attr("parent").unwrap();
        assert_eq!(value.as_ref(), Some(Some(parent_id)));
    }

    #[test]
    fn test_get_attr_unknown_returns_none() {
        let meta = Metadata::new("Test".into());
        assert!(meta.get_attr("unknown").is_none());
        assert!(meta.get_attr("").is_none());
        assert!(meta.get_attr("is_pinned").is_none()); // Uses field name, not attr name
    }

    // --- set_attr tests ---

    #[test]
    fn test_set_attr_pinned_true() {
        let mut meta = Metadata::new("Test".into());

        let effect = meta
            .set_attr("pinned", crate::attributes::AttrValue::Bool(true))
            .unwrap();

        assert!(meta.is_pinned);
        assert!(meta.pinned_at.is_some());
        assert!(meta.delete_protected); // Coupled
        assert_eq!(effect, crate::attributes::AttrSideEffect::None);
    }

    #[test]
    fn test_set_attr_pinned_false() {
        let mut meta = Metadata::new("Test".into());
        meta.is_pinned = true;
        meta.pinned_at = Some(Utc::now());
        meta.delete_protected = true;

        let effect = meta
            .set_attr("pinned", crate::attributes::AttrValue::Bool(false))
            .unwrap();

        assert!(!meta.is_pinned);
        assert!(meta.pinned_at.is_none());
        assert!(!meta.delete_protected); // Coupled
        assert_eq!(effect, crate::attributes::AttrSideEffect::None);
    }

    #[test]
    fn test_set_attr_deleted_true() {
        let mut meta = Metadata::new("Test".into());

        let effect = meta
            .set_attr("deleted", crate::attributes::AttrValue::Bool(true))
            .unwrap();

        assert!(meta.is_deleted);
        assert!(meta.deleted_at.is_some());
        assert_eq!(effect, crate::attributes::AttrSideEffect::PropagateStatusUp);
    }

    #[test]
    fn test_set_attr_deleted_false() {
        let mut meta = Metadata::new("Test".into());
        meta.is_deleted = true;
        meta.deleted_at = Some(Utc::now());

        let effect = meta
            .set_attr("deleted", crate::attributes::AttrValue::Bool(false))
            .unwrap();

        assert!(!meta.is_deleted);
        assert!(meta.deleted_at.is_none());
        assert_eq!(effect, crate::attributes::AttrSideEffect::PropagateStatusUp);
    }

    #[test]
    fn test_set_attr_protected() {
        let mut meta = Metadata::new("Test".into());

        meta.set_attr("protected", crate::attributes::AttrValue::Bool(true))
            .unwrap();
        assert!(meta.delete_protected);

        meta.set_attr("protected", crate::attributes::AttrValue::Bool(false))
            .unwrap();
        assert!(!meta.delete_protected);
    }

    #[test]
    fn test_set_attr_status_all_variants() {
        let mut meta = Metadata::new("Test".into());

        let effect = meta
            .set_attr("status", crate::attributes::AttrValue::Enum("Done".into()))
            .unwrap();
        assert_eq!(meta.status, TodoStatus::Done);
        assert_eq!(effect, crate::attributes::AttrSideEffect::PropagateStatusUp);

        meta.set_attr(
            "status",
            crate::attributes::AttrValue::Enum("InProgress".into()),
        )
        .unwrap();
        assert_eq!(meta.status, TodoStatus::InProgress);

        meta.set_attr(
            "status",
            crate::attributes::AttrValue::Enum("Planned".into()),
        )
        .unwrap();
        assert_eq!(meta.status, TodoStatus::Planned);
    }

    #[test]
    fn test_set_attr_status_invalid() {
        let mut meta = Metadata::new("Test".into());

        let result = meta.set_attr(
            "status",
            crate::attributes::AttrValue::Enum("Invalid".into()),
        );
        assert!(result.is_none());
        assert_eq!(meta.status, TodoStatus::Planned); // Unchanged
    }

    #[test]
    fn test_set_attr_tags() {
        let mut meta = Metadata::new("Test".into());
        let tags = vec!["work".to_string(), "rust".to_string()];

        let effect = meta
            .set_attr("tags", crate::attributes::AttrValue::List(tags.clone()))
            .unwrap();

        assert_eq!(meta.tags, tags);
        match effect {
            crate::attributes::AttrSideEffect::ValidateTags(t) => {
                assert_eq!(t, vec!["work".to_string(), "rust".to_string()]);
            }
            _ => panic!("Expected ValidateTags"),
        }
    }

    #[test]
    fn test_set_attr_parent() {
        let mut meta = Metadata::new("Test".into());
        let parent_id = Uuid::new_v4();

        let effect = meta
            .set_attr("parent", crate::attributes::AttrValue::Ref(Some(parent_id)))
            .unwrap();

        assert_eq!(meta.parent_id, Some(parent_id));
        assert_eq!(effect, crate::attributes::AttrSideEffect::PropagateStatusUp);
    }

    #[test]
    fn test_set_attr_parent_none() {
        let mut meta = Metadata::new("Test".into());
        meta.parent_id = Some(Uuid::new_v4());

        let effect = meta
            .set_attr("parent", crate::attributes::AttrValue::Ref(None))
            .unwrap();

        assert_eq!(meta.parent_id, None);
        assert_eq!(effect, crate::attributes::AttrSideEffect::PropagateStatusUp);
    }

    #[test]
    fn test_set_attr_unknown_returns_none() {
        let mut meta = Metadata::new("Test".into());
        let result = meta.set_attr("unknown", crate::attributes::AttrValue::Bool(true));
        assert!(result.is_none());
    }

    #[test]
    fn test_set_attr_wrong_type_returns_none() {
        let mut meta = Metadata::new("Test".into());

        // Try to set pinned with an Enum value
        let result = meta.set_attr("pinned", crate::attributes::AttrValue::Enum("yes".into()));
        assert!(result.is_none());
        assert!(!meta.is_pinned); // Unchanged

        // Try to set status with a Bool value
        let result = meta.set_attr("status", crate::attributes::AttrValue::Bool(true));
        assert!(result.is_none());
        assert_eq!(meta.status, TodoStatus::Planned); // Unchanged
    }
}
