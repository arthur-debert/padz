use crate::commands::{CmdMessage, CmdResult, PadzPaths};
use crate::error::{PadzError, Result};
use crate::model::Scope;
use std::fs;
use std::path::Path;

pub fn run(paths: &PadzPaths, scope: Scope) -> Result<CmdResult> {
    let dir = paths.scope_dir(scope)?;

    // Create scope root and bucket subdirectories
    fs::create_dir_all(dir.join("active"))?;
    fs::create_dir_all(dir.join("archived"))?;
    fs::create_dir_all(dir.join("deleted"))?;

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!(
        "Initialized padz store at {}",
        dir.display()
    )));

    // Add shell completion hint
    result.add_message(CmdMessage::info(String::new())); // blank line
    result.add_message(CmdMessage::info(
        "Tip: Enable shell completions for padz:".to_string(),
    ));
    result.add_message(CmdMessage::info(
        "  eval \"$(padz completions bash)\"  # add to ~/.bashrc".to_string(),
    ));
    result.add_message(CmdMessage::info(
        "  eval \"$(padz completions zsh)\"   # add to ~/.zshrc".to_string(),
    ));

    Ok(result)
}

/// Create a persistent link from the current project's `.padz/` to another project's data.
///
/// This writes an absolute path into `.padz/link` so that all subsequent padz invocations
/// in the current directory transparently use the target project's data store.
///
/// `local_padz` is the **pre-resolution** `.padz/` directory (i.e., the CWD-based one,
/// before any existing link is followed).
pub fn link(local_padz: &Path, target: &Path) -> Result<CmdResult> {
    // Canonicalize target
    let target = target.canonicalize().map_err(|_| {
        PadzError::Store(format!(
            "Target path '{}' does not exist or is not accessible",
            target.display()
        ))
    })?;

    // Determine target .padz dir
    let target_padz = if target.file_name().is_some_and(|n| n == ".padz") {
        target.clone()
    } else {
        target.join(".padz")
    };

    // Validate target has been initialized
    if !target_padz.join("active").exists() {
        return Err(PadzError::Store(format!(
            "Target '{}' has not been initialized. Run `padz init` there first.",
            target_padz.display()
        )));
    }

    // Reject chained links
    if target_padz.join("link").exists() {
        return Err(PadzError::Store(format!(
            "Target '{}' is itself a link. Chained links are not supported.",
            target_padz.display()
        )));
    }

    // Create local .padz/ dir if needed
    fs::create_dir_all(local_padz)?;

    // Write the link file — store the project root (parent of .padz)
    let target_root = target_padz.parent().unwrap_or(&target_padz);
    fs::write(
        local_padz.join("link"),
        target_root.to_string_lossy().as_bytes(),
    )?;

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!(
        "Linked to {}",
        target_padz.display()
    )));
    Ok(result)
}

/// Remove an existing link file.
///
/// `local_padz` is the **pre-resolution** `.padz/` directory.
pub fn unlink(local_padz: &Path) -> Result<CmdResult> {
    let link_file = local_padz.join("link");

    if !link_file.exists() {
        return Err(PadzError::Store(
            "No link exists in this project.".to_string(),
        ));
    }

    fs::remove_file(&link_file)?;

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success("Unlinked.".to_string()));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn init_padz_dir(dir: &Path) {
        fs::create_dir_all(dir.join("active")).unwrap();
        fs::create_dir_all(dir.join("archived")).unwrap();
        fs::create_dir_all(dir.join("deleted")).unwrap();
    }

    #[test]
    fn test_link_creates_link_file() {
        let temp = TempDir::new().unwrap();

        // Set up target with initialized .padz
        let target = temp.path().join("project-a");
        fs::create_dir_all(&target).unwrap();
        init_padz_dir(&target.join(".padz"));

        // Set up source
        let source_padz = temp.path().join("project-b").join(".padz");
        fs::create_dir_all(&source_padz).unwrap();

        let result = link(&source_padz, &target).unwrap();

        assert!(source_padz.join("link").exists());
        let link_content = fs::read_to_string(source_padz.join("link")).unwrap();
        assert_eq!(
            PathBuf::from(link_content.trim()),
            target.canonicalize().unwrap()
        );
        assert!(result.messages[0].content.contains("Linked to"));
    }

    #[test]
    fn test_link_validates_target_exists() {
        let temp = TempDir::new().unwrap();

        let source_padz = temp.path().join("project-b").join(".padz");
        fs::create_dir_all(&source_padz).unwrap();

        let result = link(&source_padz, &temp.path().join("nonexistent"));

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_link_validates_target_initialized() {
        let temp = TempDir::new().unwrap();

        // Target exists but not initialized (no active/ dir)
        let target = temp.path().join("project-a");
        fs::create_dir_all(target.join(".padz")).unwrap();

        let source_padz = temp.path().join("project-b").join(".padz");
        fs::create_dir_all(&source_padz).unwrap();

        let result = link(&source_padz, &target);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not been initialized"));
    }

    #[test]
    fn test_link_rejects_chain() {
        let temp = TempDir::new().unwrap();

        // Target is itself a link
        let target = temp.path().join("project-a");
        init_padz_dir(&target.join(".padz"));
        fs::write(target.join(".padz").join("link"), "/some/path").unwrap();

        let source_padz = temp.path().join("project-b").join(".padz");
        fs::create_dir_all(&source_padz).unwrap();

        let result = link(&source_padz, &target);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("itself a link"));
    }

    #[test]
    fn test_unlink_removes_link_file() {
        let temp = TempDir::new().unwrap();

        let padz_dir = temp.path().join(".padz");
        fs::create_dir_all(&padz_dir).unwrap();
        fs::write(padz_dir.join("link"), "/some/path").unwrap();

        let result = unlink(&padz_dir).unwrap();

        assert!(!padz_dir.join("link").exists());
        assert!(result.messages[0].content.contains("Unlinked"));
    }

    #[test]
    fn test_unlink_errors_when_no_link() {
        let temp = TempDir::new().unwrap();

        let padz_dir = temp.path().join(".padz");
        fs::create_dir_all(&padz_dir).unwrap();

        let result = unlink(&padz_dir);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No link exists"));
    }
}
