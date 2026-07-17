//! The editor *buffer* format — the wire format between a pad and whatever
//! text editor a user edits it in.
//!
//! Only the format lives here: turning a pad's title and content into a buffer
//! and parsing one back. Choosing an editor and launching it are user-environment
//! concerns owned by the application (see `padz::cli::editor` in the CLI crate),
//! not by this library.

use crate::model::extract_title_and_body;

/// Represents the content parsed from an editor buffer.
/// Format: title\n\ncontent
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorContent {
    pub title: String,
    pub content: String,
}

impl EditorContent {
    /// Creates an EditorContent from a title and content.
    pub fn new(title: String, content: String) -> Self {
        Self { title, content }
    }

    /// Formats the content for the editor buffer.
    /// Format: title\n\ncontent
    pub fn to_buffer(&self) -> String {
        if self.content.is_empty() {
            format!("{}\n\n", self.title)
        } else {
            format!("{}\n\n{}", self.title, self.content)
        }
    }

    /// Parses an editor buffer back into title and content.
    /// Format: first line is title, skip blank line, rest is content.
    pub fn from_buffer(buffer: &str) -> Self {
        if let Some((title, body)) = extract_title_and_body(buffer) {
            return Self {
                title,
                content: body,
            };
        }

        Self {
            title: String::new(),
            content: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_content_to_buffer_with_content() {
        let ec = EditorContent::new("My Title".to_string(), "Some content here.".to_string());
        assert_eq!(ec.to_buffer(), "My Title\n\nSome content here.");
    }

    #[test]
    fn test_editor_content_to_buffer_empty_content() {
        let ec = EditorContent::new("My Title".to_string(), String::new());
        assert_eq!(ec.to_buffer(), "My Title\n\n");
    }

    #[test]
    fn test_editor_content_from_buffer_normal() {
        let buffer = "My Title\n\nThis is content.\nMore content.";
        let ec = EditorContent::from_buffer(buffer);
        assert_eq!(ec.title, "My Title");
        assert_eq!(ec.content, "This is content.\nMore content.");
    }

    #[test]
    fn test_editor_content_from_buffer_empty_content() {
        let buffer = "My Title\n\n";
        let ec = EditorContent::from_buffer(buffer);
        assert_eq!(ec.title, "My Title");
        assert_eq!(ec.content, "");
    }

    #[test]
    fn test_editor_content_from_buffer_title_only() {
        let buffer = "My Title";
        let ec = EditorContent::from_buffer(buffer);
        assert_eq!(ec.title, "My Title");
        assert_eq!(ec.content, "");
    }

    #[test]
    fn test_editor_content_from_buffer_empty() {
        let buffer = "";
        let ec = EditorContent::from_buffer(buffer);
        assert_eq!(ec.title, "");
        assert_eq!(ec.content, "");
    }

    #[test]
    fn test_editor_content_from_buffer_no_blank_separator() {
        // If there's no blank line, content starts immediately after title
        let buffer = "Title\nContent without blank";
        let ec = EditorContent::from_buffer(buffer);
        assert_eq!(ec.title, "Title");
        assert_eq!(ec.content, "Content without blank");
    }

    #[test]
    fn test_roundtrip() {
        let original = EditorContent::new(
            "Test Title".to_string(),
            "Test content\nwith lines".to_string(),
        );
        let buffer = original.to_buffer();
        let parsed = EditorContent::from_buffer(&buffer);
        assert_eq!(original, parsed);
    }
}
