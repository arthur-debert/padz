use crate::error::{PadzError, Result};
use std::env;
use std::fs;
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
        let lines: Vec<&str> = buffer.lines().collect();

        if lines.is_empty() {
            return Self {
                title: String::new(),
                content: String::new(),
            };
        }

        let title = lines[0].to_string();

        // Find where content starts (after blank line following title)
        let content = if lines.len() > 2 && lines[1].is_empty() {
            lines[2..].join("\n")
        } else if lines.len() > 1 && lines[1].is_empty() {
            String::new()
        } else if lines.len() > 1 {
            // No blank line separator, treat rest as content
            lines[1..].join("\n")
        } else {
            String::new()
        };

        Self { title, content }
    }
}

/// Gets the editor command from environment.
/// Checks $EDITOR, then $VISUAL, then falls back to common editors.
pub fn get_editor() -> Result<String> {
    if let Ok(editor) = env::var("EDITOR")
        && !editor.is_empty()
    {
        return Ok(editor);
    }

    if let Ok(editor) = env::var("VISUAL")
        && !editor.is_empty()
    {
        return Ok(editor);
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
/// Returns the contents of the file after editing.
pub fn open_in_editor<P: AsRef<Path>>(file_path: P) -> Result<String> {
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

    fs::read_to_string(path).map_err(PadzError::Io)
}

/// Opens an editor with initial content and returns the edited content.
/// Creates a temporary file with the given extension.
pub fn edit_content(initial: &EditorContent, file_extension: &str) -> Result<EditorContent> {
    let temp_dir = env::temp_dir();
    let temp_file = temp_dir.join(format!("padz_edit{}", file_extension));

    // Write initial content
    fs::write(&temp_file, initial.to_buffer()).map_err(PadzError::Io)?;

    // Open editor
    let result = open_in_editor(&temp_file)?;

    // Clean up temp file
    let _ = fs::remove_file(&temp_file);

    Ok(EditorContent::from_buffer(&result))
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
