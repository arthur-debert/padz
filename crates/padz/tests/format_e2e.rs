#![allow(deprecated)]

//! # Pad file formats — subprocess E2E
//!
//! ## The real boundary this file protects
//!
//! **`padz config set format`, which `cli::run` handles before dispatch.**
//!
//! This suite turns on the config *write/load* path: `config` is intercepted
//! by `run` and handed to clapfig, never reaching a handler, so
//! `App::run_to_string` — where `TestHarness` starts — cannot reach it. The
//! tests that flip `format` in config and then observe the next create honor it
//! (`test_config_set_format_affects_new_pads`,
//! `test_config_set_format_does_not_rename_existing_pads`,
//! `test_config_with_stale_keys_still_loads_format`) need that round trip.
//!
//! Direct format behavior lives in `padzapp/tests/format_behavior.rs`; typed
//! handler forwarding of `format` lives in `handlers_direct.rs`.

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
fn test_config_set_format_affects_new_pads() {
    // Process-only boundary: `config set` writes through clapfig before dispatch,
    // and a later invocation must reload that persisted value into AppState.
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
    // Process-only boundary: a pre-dispatch `config set` must persist for later
    // invocations without rewriting files created by an earlier invocation.
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
fn test_config_with_stale_keys_still_loads_format() {
    // Process-only boundary: startup configuration loading happens before
    // dispatch and must tolerate stale keys while hydrating the active format.
    let temp = TempDir::new().unwrap();
    let (project, global_dir) = setup_project(&temp);

    // Write a config file with stale/unknown keys (simulates schema evolution)
    let config_path = project.join(".padz").join("padz.toml");
    fs::write(
        &config_path,
        "modes = \"todos\"\nmode = \"todos\"\nformat = \"lex\"\n",
    )
    .unwrap();

    // Create a pad — should use "lex" from config despite the stale "modes" key
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["create", "--no-editor", "stale key test"])
        .assert()
        .success();

    let active_dir = project.join(".padz").join("active");
    let lex_count = fs::read_dir(&active_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            name.starts_with("pad-") && name.ends_with(".lex")
        })
        .count();

    assert_eq!(
        lex_count, 1,
        "Config with stale keys should still apply format = lex"
    );
}
