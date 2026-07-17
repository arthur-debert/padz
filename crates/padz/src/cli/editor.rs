//! Launching the user's text editor.
//!
//! This is a user-environment concern and therefore lives in the CLI, not in
//! `padzapp`: it reads `$EDITOR`/`$VISUAL`, probes `PATH` for fallbacks, and
//! spawns a child process that takes over the terminal. The library owns only
//! the buffer format ([`padzapp::editor::EditorContent`]).
//!
//! Selection is separated from launching so it can be tested without spawning
//! anything: [`select_editor`] is a pure function of an [`EditorEnv`], and only
//! [`open_in_editor`] touches the process table.

use padzapp::error::{PadzError, Result};
use std::path::Path;
use std::process::Command;

/// Editors tried, in order, when neither `$EDITOR` nor `$VISUAL` is set.
const FALLBACK_EDITORS: [&str; 3] = ["vim", "vi", "nano"];

/// The environment inputs editor selection depends on.
///
/// Modeled as data so the selection rules can be tested directly, rather than
/// by mutating the process environment (which is global and racy under a
/// parallel test runner).
pub struct EditorEnv {
    /// The value of `$EDITOR`, if set and non-empty.
    pub editor: Option<String>,
    /// The value of `$VISUAL`, if set and non-empty.
    pub visual: Option<String>,
    /// Is `name` an executable on `PATH`? Used to pick among the fallbacks.
    pub is_on_path: fn(&str) -> bool,
}

impl EditorEnv {
    /// Reads the real process environment. The composition root for editing.
    pub fn from_process() -> Self {
        Self {
            editor: non_empty_var("EDITOR"),
            visual: non_empty_var("VISUAL"),
            is_on_path: which,
        }
    }
}

/// Reads `name` from the environment, treating an empty value as unset —
/// `EDITOR=` should fall through to `$VISUAL` rather than trying to run "".
fn non_empty_var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// Is `name` an executable on `PATH`?
fn which(name: &str) -> bool {
    Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Picks the editor command to run: `$EDITOR`, then `$VISUAL`, then the first
/// of [`FALLBACK_EDITORS`] found on `PATH`.
pub fn select_editor(env: &EditorEnv) -> Result<String> {
    if let Some(editor) = env.editor.clone() {
        return Ok(editor);
    }
    if let Some(visual) = env.visual.clone() {
        return Ok(visual);
    }
    for fallback in FALLBACK_EDITORS {
        if (env.is_on_path)(fallback) {
            return Ok(fallback.to_string());
        }
    }
    Err(PadzError::Api(
        "No editor found. Set $EDITOR environment variable.".to_string(),
    ))
}

/// Opens `file_path` in the user's editor and waits for it to close.
///
/// Errors when no editor can be selected, when the editor cannot be spawned,
/// or when it exits non-zero (which we treat as "the user aborted the edit").
pub fn open_in_editor<P: AsRef<Path>>(file_path: P) -> Result<()> {
    let editor = select_editor(&EditorEnv::from_process())?;
    open_with(&editor, file_path.as_ref())
}

/// Runs `editor` against `path` and waits for it to close.
///
/// Split out from [`open_in_editor`] so the spawn-and-wait behavior can be
/// tested against a real child process without touching `$EDITOR` (which is
/// process-global, and so racy to mutate under a parallel test runner).
pub fn open_with(editor: &str, path: &Path) -> Result<()> {
    let status = Command::new(editor)
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

    /// An env with nothing set and nothing on PATH.
    fn bare_env() -> EditorEnv {
        EditorEnv {
            editor: None,
            visual: None,
            is_on_path: |_| false,
        }
    }

    #[test]
    fn editor_var_wins() {
        let env = EditorEnv {
            editor: Some("emacs".into()),
            visual: Some("code".into()),
            ..bare_env()
        };
        assert_eq!(select_editor(&env).unwrap(), "emacs");
    }

    #[test]
    fn visual_used_when_editor_unset() {
        let env = EditorEnv {
            visual: Some("code".into()),
            ..bare_env()
        };
        assert_eq!(select_editor(&env).unwrap(), "code");
    }

    #[test]
    fn falls_back_to_first_editor_on_path() {
        // `vim` absent, `vi` present → `vi` wins over the later `nano`.
        let env = EditorEnv {
            is_on_path: |name| name != "vim",
            ..bare_env()
        };
        assert_eq!(select_editor(&env).unwrap(), "vi");
    }

    #[test]
    fn errors_when_nothing_available() {
        let err = select_editor(&bare_env()).unwrap_err().to_string();
        assert!(err.contains("No editor found"), "got: {err}");
        assert!(err.contains("$EDITOR"), "got: {err}");
    }

    // --- open_with: real child processes, no TTY and no $EDITOR needed ---

    /// Writes an executable shell script and returns its path.
    fn script(dir: &std::path::Path, name: &str, body: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, body).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        path
    }

    /// The happy path: the editor is handed the pad's path, and whatever it
    /// writes there is what lands on disk.
    #[cfg(unix)]
    #[test]
    fn open_with_runs_editor_against_the_file() {
        let temp = tempfile::tempdir().unwrap();
        let pad = temp.path().join("pad.txt");
        std::fs::write(&pad, "before").unwrap();
        let editor = script(
            temp.path(),
            "ed.sh",
            "#!/bin/sh\nprintf 'Edited Title\\n\\nEdited body.' > \"$1\"\n",
        );

        open_with(editor.to_str().unwrap(), &pad).unwrap();

        assert_eq!(
            std::fs::read_to_string(&pad).unwrap(),
            "Edited Title\n\nEdited body."
        );
    }

    /// A non-zero exit means the user bailed out of the edit; that must surface
    /// as an error rather than silently accepting the buffer.
    #[cfg(unix)]
    #[test]
    fn open_with_errors_when_editor_exits_non_zero() {
        let temp = tempfile::tempdir().unwrap();
        let pad = temp.path().join("pad.txt");
        std::fs::write(&pad, "before").unwrap();
        let editor = script(temp.path(), "fail.sh", "#!/bin/sh\nexit 1\n");

        let err = open_with(editor.to_str().unwrap(), &pad).unwrap_err();
        assert!(
            err.to_string().contains("exited with non-zero status"),
            "got: {err}"
        );
    }

    /// An editor that isn't there at all reports the launch failure, naming the
    /// command so the user can see what their `$EDITOR` pointed at.
    #[test]
    fn open_with_errors_when_editor_missing() {
        let temp = tempfile::tempdir().unwrap();
        let pad = temp.path().join("pad.txt");
        std::fs::write(&pad, "before").unwrap();

        let err = open_with("padz-no-such-editor-binary", &pad).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Failed to launch editor"), "got: {msg}");
        assert!(msg.contains("padz-no-such-editor-binary"), "got: {msg}");
    }
}
