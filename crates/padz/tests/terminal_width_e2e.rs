#![allow(deprecated)]

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn padz_cmd() -> Command {
    Command::new(cargo_bin("padz"))
}

/// Helper to set up a project with git + padz init + some pads
fn setup_project_with_pads(temp: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
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

    for title in &[
        "Short",
        "A medium-length title for testing",
        "This is a much longer title that should get truncated when the terminal is narrow enough",
    ] {
        padz_cmd()
            .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
            .current_dir(&project)
            .args(["create", "--no-editor", title])
            .assert()
            .success();
    }

    (project, global_dir)
}

/// Verify that JSON output works at various COLUMNS widths (JSON bypasses templates).
/// This confirms the COLUMNS env var is respected and padz doesn't crash.
#[test]
fn test_list_json_works_at_various_columns() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project_with_pads(&temp);

    for columns in [30, 40, 60, 80, 120] {
        let output = padz_cmd()
            .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
            .env("COLUMNS", columns.to_string())
            .current_dir(&project)
            .args(["list", "--output", "json"])
            .output()
            .expect("failed to run padz list");

        assert!(
            output.status.success(),
            "COLUMNS={columns}: padz list --output json failed"
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        let data: serde_json::Value = serde_json::from_str(&stdout).expect("JSON parse failed");

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(pads.len(), 3, "COLUMNS={columns}: expected 3 pads");
    }
}

/// Verify search JSON output also works at various widths.
#[test]
fn test_search_json_works_at_various_columns() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project_with_pads(&temp);

    for columns in [30, 80] {
        let output = padz_cmd()
            .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
            .env("COLUMNS", columns.to_string())
            .current_dir(&project)
            .args(["search", "title", "--output", "json"])
            .output()
            .expect("failed to run padz search");

        assert!(
            output.status.success(),
            "COLUMNS={columns}: padz search --output json failed"
        );
    }
}
