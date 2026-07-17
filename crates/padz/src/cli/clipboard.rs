//! Reading and writing the system clipboard.
//!
//! A user-environment concern owned by the CLI: every path here shells out to
//! a platform clipboard tool (`pbcopy`/`pbpaste`, `xclip`/`xsel`, `clip`/
//! PowerShell). The library never touches the clipboard — handlers hand pad
//! text to these functions at the CLI boundary.

use padzapp::error::{PadzError, Result};
use std::process::Command;

/// Copies text to the system clipboard in an OS-specific way.
/// - macOS: uses pbcopy
/// - Linux: uses xclip or xsel
/// - Windows: uses clip.exe
pub fn copy_to_clipboard(text: &str) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        copy_macos(text)
    }

    #[cfg(target_os = "linux")]
    {
        copy_linux(text)
    }

    #[cfg(target_os = "windows")]
    {
        copy_windows(text)
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(PadzError::Api(
            "Clipboard not supported on this platform".to_string(),
        ))
    }
}

#[cfg(target_os = "macos")]
fn copy_macos(text: &str) -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("pbcopy")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| PadzError::Api(format!("Failed to spawn pbcopy: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| PadzError::Api(format!("Failed to write to pbcopy: {}", e)))?;
    }

    let status = child
        .wait()
        .map_err(|e| PadzError::Api(format!("Failed to wait for pbcopy: {}", e)))?;

    if status.success() {
        Ok(())
    } else {
        Err(PadzError::Api("pbcopy exited with error".to_string()))
    }
}

#[cfg(target_os = "linux")]
fn copy_linux(text: &str) -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    // Try xclip first, then xsel
    let result = Command::new("xclip")
        .args(["-selection", "clipboard"])
        .stdin(Stdio::piped())
        .spawn();

    let mut child = match result {
        Ok(child) => child,
        Err(_) => {
            // Try xsel as fallback
            Command::new("xsel")
                .args(["--clipboard", "--input"])
                .stdin(Stdio::piped())
                .spawn()
                .map_err(|e| {
                    PadzError::Api(format!(
                        "Failed to spawn xclip or xsel: {}. Install xclip or xsel.",
                        e
                    ))
                })?
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| PadzError::Api(format!("Failed to write to clipboard: {}", e)))?;
    }

    let status = child
        .wait()
        .map_err(|e| PadzError::Api(format!("Failed to wait for clipboard command: {}", e)))?;

    if status.success() {
        Ok(())
    } else {
        Err(PadzError::Api(
            "Clipboard command exited with error".to_string(),
        ))
    }
}

#[cfg(target_os = "windows")]
fn copy_windows(text: &str) -> Result<()> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("clip")
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| PadzError::Api(format!("Failed to spawn clip: {}", e)))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(text.as_bytes())
            .map_err(|e| PadzError::Api(format!("Failed to write to clip: {}", e)))?;
    }

    let status = child
        .wait()
        .map_err(|e| PadzError::Api(format!("Failed to wait for clip: {}", e)))?;

    if status.success() {
        Ok(())
    } else {
        Err(PadzError::Api("clip exited with error".to_string()))
    }
}

/// Formats pad content for clipboard (title + blank line + content)
pub fn format_for_clipboard(title: &str, content: &str) -> String {
    if content.is_empty() {
        format!("{}\n\n", title)
    } else {
        format!("{}\n\n{}", title, content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_for_clipboard_with_content() {
        let result = format_for_clipboard("My Title", "Some content");
        assert_eq!(result, "My Title\n\nSome content");
    }

    /// The real platform clipboard round-trip.
    ///
    /// Ignored by default: it depends on a working clipboard tool (and, on
    /// Linux, an X display), which CI has no business requiring. Run it with
    /// `cargo test -p padz -- --ignored` on a desktop to check this adapter
    /// against the actual system clipboard.
    #[test]
    #[ignore = "requires a working system clipboard"]
    fn copy_round_trips_through_the_system_clipboard() {
        let text = "padz clipboard adapter test";
        copy_to_clipboard(text).unwrap();
    }

    #[test]
    fn test_format_for_clipboard_empty_content() {
        let result = format_for_clipboard("My Title", "");
        assert_eq!(result, "My Title\n\n");
    }
}
