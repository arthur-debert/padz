use crate::error::{PadzError, Result};
use crate::model::parse_pad_content;
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
        if let Some((title, full_content)) = parse_pad_content(buffer) {
            // parse_pad_content returns (title, full_title_and_content)
            // But EditorContent seems to want (title, body) to separate?
            // Wait, parse_pad_content returns (Title, FullContent).
            // EditorContent currently stores `content` as Body.
            // If I change EditorContent semantics, I break main.rs logic?
            // Let's see. main.rs uses `edited.title` and `edited.content`.
            // And `Pad::new(title, content)` expects `content` to be Body?
            // No, Pad::new implementation I just wrote:
            // normalize_pad_content(title, content) -> returns Full Content.
            // It assumes `content` passed to it is BODY.
            // So if `edited.content` is Full Content, `Pad::new` will produce Double Title?

            // normalize_pad_content(title, body) -> (title, Title\n\nBody).

            // If parse_pad_content returns (Title, FullString).
            // We need to extract Body from FullString?
            // Or better, let parse_pad_content return (Title, Body)?
            // normalize_pad_content returns (Title, FullString).

            // Let's look at `model.rs` again.
            // `parse_pad_content` calls `normalize_pad_content`.
            // `normalize_pad_content` returns `full_content`.

            // If I want to split them effectively, I should maybe have `parse` return `(Title, Body)`?
            // `parse_pad_content` in my implementation:
            // extracts title, extracts body_raw.
            // calls normalize -> returns (Title, FullCanonical).

            // So I don't have a clean way to get canonical Body from `model` yet.
            // I should modify `model.rs` to expose body?

            // Actually, `EditorContent` is "Title" and "Content".
            // If I want to keep semantics of "Content" = "Body", I need to extract body.
            // But `Pad` now stores Full Content.

            // If I change `EditorContent` sematics to "Content" = "Full Content",
            // then `to_buffer` should just print `content` (since it has title).
            // And `from_buffer` should just return `(parsed_title, buffer)`.

            // Let's modify EditorContent semantics to align with Pad.
            // `EditorContent.content` = Full Normalized Content.
            // `to_buffer` -> returns `.content`.
            // `from_buffer` -> `parse_pad_content(buf)`.
            // `new` -> `normalize_pad_content(title, body)`.

            // This seems consistent.

            // Check main.rs usage:
            // create: new(title, body_arg). -> OK.
            // edit_content calls to_buffer. -> Returns Full.
            // edit returns edited EditorContent.
            // main calls `api.create_pad(scope, edited.title, edited.content)`.
            // `api.create_pad` calls `Pad::new(title, content)`.
            // `Pad::new` calls `normalize_pad_content(title, content)`.

            // IF `edited.content` is Full Content (Title\n\nBody).
            // `normalize` will treat it as Body!
            // Result: Title\n\n(Title\n\nBody). Double Title!

            // So Pad::new expects Body.
            // So EditorContent should store Body?

            // OR Pad::new should be smart?
            // No, Pad::new(title, body) is standard API.

            // So EditorContent MUST store Body.

            // I need `model::parse` to return Body.
            // My `model::parse_pad_content` currently returns Full Content.

            // I will modify `EditorContent.from_buffer` to strip title from full content.
            // Or better, modify `model.rs` to return `(Title, Body)` as well?

            // Let's parse manually in `from_buffer` using `model` helpers if possible.
            // Or just do `parse_pad_content` then strip title?

            let (p_title, p_full) = match parse_pad_content(buffer) {
                Some(res) => res,
                None => {
                    return Self {
                        title: String::new(),
                        content: String::new(),
                    }
                }
            };

            // strip title from p_full?
            // format is Title\n\nBody or Title.
            let body = if p_full.starts_with(&p_title) {
                p_full[p_title.len()..].trim().to_string()
            } else {
                String::new() // Should not happen if normalized
            };

            Self {
                title: p_title,
                content: body,
            }
        } else {
            Self {
                title: String::new(),
                content: String::new(),
            }
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
