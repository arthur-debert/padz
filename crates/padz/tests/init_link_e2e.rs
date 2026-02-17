#![allow(deprecated)]

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn padz_cmd() -> Command {
    Command::new(cargo_bin("padz"))
}

#[test]
fn test_init_link_full_workflow() {
    let temp = TempDir::new().unwrap();
    let project_a = temp.path().join("project-a");
    let project_b = temp.path().join("project-b");

    // Create git repos
    fs::create_dir_all(&project_a).unwrap();
    fs::create_dir_all(&project_b).unwrap();
    fs::create_dir(project_a.join(".git")).unwrap();
    fs::create_dir(project_b.join(".git")).unwrap();

    let global_dir = temp.path().join("global");
    fs::create_dir_all(&global_dir).unwrap();

    // 1. Init project-a
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project_a)
        .args(["init"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Initialized"));

    // 2. Create a pad in project-a
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project_a)
        .args(["create", "--no-editor", "hello from A"])
        .assert()
        .success();

    // 3. Link project-b to project-a
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project_b)
        .args(["init", "--link", project_a.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("Linked to"));

    // Verify link file was created
    assert!(project_b.join(".padz").join("link").exists());

    // 4. List from project-b — should see project-a's pad
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project_b)
        .args(["list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("hello from A"));

    // 5. Create a pad from project-b — should appear in project-a's store
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project_b)
        .args(["create", "--no-editor", "from B via link"])
        .assert()
        .success();

    // Verify it shows in project-a's listing
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project_a)
        .args(["list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::contains("from B via link"));

    // 6. Unlink
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project_b)
        .args(["init", "--unlink"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Unlinked"));

    // Verify link file removed
    assert!(!project_b.join(".padz").join("link").exists());

    // 7. List from project-b after unlink — should be empty (no local data)
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project_b)
        .args(["list", "--output", "json"])
        .assert()
        .success()
        .stdout(predicate::str::is_empty().or(predicate::str::contains("hello from A").not()));
}

#[test]
fn test_init_link_rejects_nonexistent_target() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir(project.join(".git")).unwrap();

    let global_dir = temp.path().join("global");
    fs::create_dir_all(&global_dir).unwrap();

    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["init", "--link", "/nonexistent/path"])
        .assert()
        .failure();
}

#[test]
fn test_init_link_rejects_uninitialized_target() {
    let temp = TempDir::new().unwrap();
    let target = temp.path().join("target");
    let source = temp.path().join("source");
    fs::create_dir_all(&target).unwrap();
    fs::create_dir_all(&source).unwrap();
    fs::create_dir(source.join(".git")).unwrap();
    // Target has no .padz at all

    let global_dir = temp.path().join("global");
    fs::create_dir_all(&global_dir).unwrap();

    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&source)
        .args(["init", "--link", target.to_str().unwrap()])
        .assert()
        .failure();
}

#[test]
fn test_init_unlink_errors_when_no_link() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir(project.join(".git")).unwrap();

    let global_dir = temp.path().join("global");
    fs::create_dir_all(&global_dir).unwrap();

    // Init without link
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["init"])
        .assert()
        .success();

    // Unlink should fail
    padz_cmd()
        .env("PADZ_GLOBAL_DATA", global_dir.as_os_str())
        .current_dir(&project)
        .args(["init", "--unlink"])
        .assert()
        .failure();
}

#[test]
fn test_init_link_and_unlink_conflict() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();

    // --link and --unlink should conflict
    padz_cmd()
        .current_dir(&project)
        .args(["init", "--link", "/some/path", "--unlink"])
        .assert()
        .failure();
}
