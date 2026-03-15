#![allow(deprecated)]

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn padz_cmd() -> Command {
    Command::new(cargo_bin("padz"))
}

/// Helper to set up a project with git + padz init
fn setup_project(temp: &TempDir) -> (std::path::PathBuf, std::path::PathBuf) {
    let project = temp.path().join("project");
    let global_dir = temp.path().join("global");

    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&global_dir).unwrap();
    fs::create_dir(project.join(".git")).unwrap();

    // Init padz
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["init"])
        .assert()
        .success();

    (project, global_dir)
}

#[test]
fn test_create_uses_default_txt_format() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Create a pad (default format = txt)
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "hello"])
        .assert()
        .success();

    // Check that a .txt file was created in .padz/active/
    let active_dir = project.join(".padz").join("active");
    let entries: Vec<_> = fs::read_dir(&active_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("pad-") && name.ends_with(".txt")
        })
        .collect();

    assert_eq!(entries.len(), 1, "Expected one .txt pad file");
}

#[test]
fn test_create_with_format_override_creates_md() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Create a pad with --format md
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "--format", "md", "markdown pad"])
        .assert()
        .success();

    // Check that a .md file was created
    let active_dir = project.join(".padz").join("active");
    let entries: Vec<_> = fs::read_dir(&active_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("pad-") && name.ends_with(".md")
        })
        .collect();

    assert_eq!(entries.len(), 1, "Expected one .md pad file");
}

#[test]
fn test_format_override_does_not_affect_subsequent_creates() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Create first pad with --format md
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "--format", "md", "md pad"])
        .assert()
        .success();

    // Create second pad without format (should use default .txt)
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "txt pad"])
        .assert()
        .success();

    let active_dir = project.join(".padz").join("active");
    let md_count = fs::read_dir(&active_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("pad-") && name.ends_with(".md")
        })
        .count();
    let txt_count = fs::read_dir(&active_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("pad-") && name.ends_with(".txt")
        })
        .count();

    assert_eq!(md_count, 1, "Expected one .md file");
    assert_eq!(txt_count, 1, "Expected one .txt file");
}

#[test]
fn test_mixed_formats_list_and_view_work() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Create pads with different formats
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "--format", "md", "markdown note"])
        .assert()
        .success();

    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "plain text note"])
        .assert()
        .success();

    // List should show both
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("markdown note"))
        .stdout(predicate::str::contains("plain text note"));

    // View each should work
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["view", "1", "--output", "json"])
        .assert()
        .success();

    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["view", "2", "--output", "json"])
        .assert()
        .success();
}

#[test]
fn test_config_set_format_affects_new_pads() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Set global format to md
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["config", "set", "format", "md"])
        .assert()
        .success();

    // Create a pad (should use md from config)
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "after config change"])
        .assert()
        .success();

    let active_dir = project.join(".padz").join("active");
    let md_count = fs::read_dir(&active_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("pad-") && name.ends_with(".md")
        })
        .count();

    assert_eq!(md_count, 1, "New pad should use .md from config");
}

#[test]
fn test_config_set_format_does_not_rename_existing_pads() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Create a pad with default .txt
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "old pad"])
        .assert()
        .success();

    // Change format to md
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["config", "set", "format", "md"])
        .assert()
        .success();

    // The existing .txt pad should still be a .txt file
    let active_dir = project.join(".padz").join("active");
    let txt_count = fs::read_dir(&active_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("pad-") && name.ends_with(".txt")
        })
        .count();

    assert_eq!(
        txt_count, 1,
        "Existing .txt pad should NOT be migrated to .md"
    );

    // And the old pad should still be listable
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("old pad"));
}

#[test]
fn test_format_alias_markdown_creates_md() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Create with "markdown" alias
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args([
            "create",
            "--no-editor",
            "--format",
            "markdown",
            "alias test",
        ])
        .assert()
        .success();

    let active_dir = project.join(".padz").join("active");
    let md_count = fs::read_dir(&active_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("pad-") && name.ends_with(".md")
        })
        .count();

    assert_eq!(md_count, 1, "\"markdown\" alias should create .md file");
}

#[test]
fn test_format_alias_text_creates_txt() {
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Set format to md first
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["config", "set", "format", "md"])
        .assert()
        .success();

    // Create with "text" alias (should override config and create .txt)
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "--format", "text", "text alias"])
        .assert()
        .success();

    let active_dir = project.join(".padz").join("active");
    let txt_count = fs::read_dir(&active_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("pad-") && name.ends_with(".txt")
        })
        .count();

    assert_eq!(txt_count, 1, "\"text\" alias should create .txt file");
}
