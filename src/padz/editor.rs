use crate::error::{PadzError, Result};
use crate::model::extract_title_and_body;
use std::env;
use std::path::Path;
use std::process::Command;

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

/// Gets the editor command from environment.
/// Checks $EDITOR, then $VISUAL, then falls back to common editors.
pub fn get_editor() -> Result<String> {
    if let Ok(editor) = env::var("EDITOR") {
        if !editor.is_empty() {
            return Ok(editor);
        }
    }

    if let Ok(editor) = env::var("VISUAL") {
        if !editor.is_empty() {
            return Ok(editor);
        }
    }

    // Try common fallbacks
    for fallback in &["vim", "vi", "nano"] {
        if Command::new("which")
            .arg(fallback)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok((*fallback).to_string());
        }
    }

    Err(PadzError::Api(
        "No editor found. Set $EDITOR environment variable.".to_string(),
    ))
}

/// Opens a file in the user's editor and waits for it to close.
pub fn open_in_editor<P: AsRef<Path>>(file_path: P) -> Result<()> {
    let editor = get_editor()?;
    let path = file_path.as_ref();

    let status = Command::new(&editor)
        .arg(path)
        .status()
        .map_err(|e| PadzError::Api(format!("Failed to launch editor '{}': {}", editor, e)))?;

    if !status.success() {
        return Err(PadzError::Api(format!(
            "Editor '{}' exited with non-zero status",
            editor
        )));
    }

    Ok(())
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
