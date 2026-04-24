#![allow(deprecated)]

//! End-to-end tests for the configurable `ordering` setting.
//!
//! Exercises the full binary path: create pads, flip the config, edit a pad
//! (via piped stdin, which triggers the update flow non-interactively), and
//! assert the listing order reflects the config.

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use tempfile::TempDir;

fn padz_cmd() -> Command {
    Command::new(cargo_bin("padz"))
}

fn setup_project(temp: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let project = temp.path().join("project");
    let global_dir = temp.path().join("global");

    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&global_dir).unwrap();
    fs::create_dir(project.join(".git")).unwrap();

    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["init"])
        .assert()
        .success();

    (project, global_dir)
}

/// Parse `padz list --output json` and return the list of pad titles in the
/// order they appear in the returned `pads` array (root-level only).
fn list_titles(project: &std::path::Path, global_dir: &std::path::Path) -> Vec<String> {
    let output = padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(project)
        .args(["list", "--output", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "padz list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let value: Value = serde_json::from_str(&stdout).expect("list output should be JSON");
    let pads = value
        .get("pads")
        .and_then(|v| v.as_array())
        .expect("pads array");
    pads.iter()
        .map(|dp| {
            dp.get("pad")
                .and_then(|p| p.get("metadata"))
                .and_then(|m| m.get("title"))
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string()
        })
        .collect()
}

fn create_pad(project: &std::path::Path, global_dir: &std::path::Path, title: &str) {
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(project)
        .args(["create", "--no-editor", title])
        .assert()
        .success();
    // Space the creations so created_at / updated_at timestamps differ.
    std::thread::sleep(std::time::Duration::from_millis(20));
}

/// Update a pad by piping content to `padz open <index>`. This exercises the
/// `run_from_content` path, which updates title + body and bumps `updated_at`.
fn edit_pad(project: &std::path::Path, global_dir: &std::path::Path, index: &str, new_title: &str) {
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(project)
        .args(["open", index])
        .write_stdin(format!("{}\n\nbody\n", new_title))
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(20));
}

#[test]
fn test_default_ordering_is_creation_newest_first() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    create_pad(&project, &global_dir, "Alpha");
    create_pad(&project, &global_dir, "Beta");
    create_pad(&project, &global_dir, "Gamma");

    // Default ordering is created_at descending → newest first.
    assert_eq!(
        list_titles(&project, &global_dir),
        vec!["Gamma", "Beta", "Alpha"]
    );
}

#[test]
fn test_ordering_updated_at_reorders_after_edit() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    create_pad(&project, &global_dir, "Alpha");
    create_pad(&project, &global_dir, "Beta");
    create_pad(&project, &global_dir, "Gamma");

    // Baseline: newest-created first.
    assert_eq!(
        list_titles(&project, &global_dir),
        vec!["Gamma", "Beta", "Alpha"]
    );

    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["config", "set", "ordering", "updated_at"])
        .assert()
        .success();

    // No edits yet: updated_at ≈ created_at, so order is unchanged.
    assert_eq!(
        list_titles(&project, &global_dir),
        vec!["Gamma", "Beta", "Alpha"]
    );

    // Edit Alpha (currently at index 3 — oldest). Under updated_at it should lead.
    edit_pad(&project, &global_dir, "3", "Alpha Revised");

    assert_eq!(
        list_titles(&project, &global_dir),
        vec!["Alpha Revised", "Gamma", "Beta"],
        "Alpha should bubble to the top after edit under updated_at ordering"
    );
}

#[test]
fn test_config_toggle_back_to_created_at_restores_stable_order() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    create_pad(&project, &global_dir, "First");
    create_pad(&project, &global_dir, "Second");
    create_pad(&project, &global_dir, "Third");

    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["config", "set", "ordering", "updated_at"])
        .assert()
        .success();

    edit_pad(&project, &global_dir, "3", "First Edited");
    assert_eq!(
        list_titles(&project, &global_dir)[0],
        "First Edited",
        "under updated_at, First should lead after edit"
    );

    // Flip back to created_at: the creation sequence wins again.
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["config", "set", "ordering", "created_at"])
        .assert()
        .success();

    assert_eq!(
        list_titles(&project, &global_dir),
        vec!["Third", "Second", "First Edited"],
        "under created_at, creation sequence wins regardless of edit times"
    );
}

#[test]
fn test_nested_edit_surfaces_parent_under_updated_at() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Parent with two children, then a sibling root created afterwards.
    create_pad(&project, &global_dir, "Root With Children");
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "-i", "1", "Child One"])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(20));
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "-i", "1", "Child Two"])
        .assert()
        .success();
    std::thread::sleep(std::time::Duration::from_millis(20));
    create_pad(&project, &global_dir, "Second Root");

    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["config", "set", "ordering", "updated_at"])
        .assert()
        .success();

    // "Second Root" is newer than the parent, so it leads initially.
    let before = list_titles(&project, &global_dir);
    assert_eq!(before[0], "Second Root");

    // Edit a nested child. `propagate_modification` bumps the parent's
    // `updated_at` so it surfaces above "Second Root".
    edit_pad(&project, &global_dir, "2.1", "Child Two Edited");

    let after = list_titles(&project, &global_dir);
    assert_eq!(
        after[0], "Root With Children",
        "parent should surface when a nested child is edited (got: {:?})",
        after
    );
}
