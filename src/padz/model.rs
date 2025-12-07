//! # Padz Data Model
//!
//! This module defines the core data structures for Padz, specifically the `Pad` and `Metadata` structs.
//! It also handles the normalization logic crucial for the application's data integrity.
//!
//! ## Pad File Layout
//!
//! Padz stores pad content in plain text files with a canonical format:
//!
//! 1.  **Title**: The first line of the file.
//! 2.  **Separator**: A single blank line.
//! 3.  **Content**: The remainder of the file.
//!
//! ### Normalization Rules
//!
//! -   **Title**: Defined as the first line of content. For display purposes (metadata), it is truncated to 60 characters.
//! -   **Blank Line**: A single blank line is enforced between the title and the content body.
//! -   **Content**: Starts from the third line (if a blank line exists) to EOF.
//! -   **Whitespace**:
//!     -   Leading blank lines before the title are stripped.
//!     -   Trailing blank lines in the content are stripped (though internal blank lines are preserved).
//! -   **Validity**:
//!     -   A one-line pad (Title only) is valid.
//!     -   A pad with no text at all is invalid and should not be stored.
//!
//! ### Examples
//!
//! A standard pad:
//! ```text
//! My Title
//!
//! This is the body content.
//! It can have multiple lines.
//! ```
//!
//! A one-line pad:
//! ```text
//! Just A Title
//! ```
//!
//! The normalization logic ensures that any input is transformed into this structure before storage.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Scope {
    Project,
    Global,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub is_pinned: bool,
    pub pinned_at: Option<DateTime<Utc>>,
    pub is_deleted: bool,
    pub deleted_at: Option<DateTime<Utc>>,
    // We store the title in metadata to list without reading content files
    pub title: String,
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
            title,
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
        clean_title.chars().take(60).collect::<String>()
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
        assert_eq!(title.len(), 60);
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
}
