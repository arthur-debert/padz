#![allow(deprecated)]

//! Help/topic contract coverage, pinned during the Standout 6.2 -> 7.6 upgrade.
//!
//! Standout 7.6 requires `help_handling(true)` to be opted into explicitly (it
//! was implicit in 6.2, and 7.6 refuses to build an app that declares topics or
//! command groups without it). Enabling it also routes clap's `--help`/`-h`
//! through Standout's renderer, which 6.2 left to clap. These tests pin the
//! resulting contract so a later workstream cannot regress it silently.

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use predicates::prelude::*;

fn padz_cmd() -> Command {
    Command::new(cargo_bin("padz"))
}

/// The `help` subcommand must exist. It is installed by `help_handling(true)`;
/// without that opt-in Standout 7.6 drops it and the command-group validation
/// in `cli::setup` fails on the "Misc" group.
#[test]
fn test_help_subcommand_exists() {
    padz_cmd()
        .args(["help", "create"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Create a new pad"));
}

/// Help topics must still resolve through `help <topic>`.
#[test]
fn test_help_topic_renders() {
    padz_cmd()
        .args(["help", "scopes"])
        .assert()
        .success()
        .stdout(predicate::str::contains("PROJECTS AND GLOBAL NOTES"));
}

/// An unknown help keyword is neither a command nor a topic and must fail.
#[test]
fn test_help_unknown_keyword_errors() {
    padz_cmd().args(["help", "nosuchtopic"]).assert().failure();
}

/// Top-level help keeps padz's grouped layout, including the "LEARN MORE"
/// topics section.
#[test]
fn test_root_help_keeps_groups_and_topics() {
    padz_cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("PER PAD(S)"))
        .stdout(predicate::str::contains("LEARN MORE"))
        .stdout(predicate::str::contains("scopes:"));
}

/// Under 7.6, `<cmd> --help` is rendered by Standout rather than clap, so it now
/// matches `help <cmd>` byte for byte. Under 6.2 these two disagreed: `help
/// create` was Standout-rendered while `create --help` was clap-rendered.
#[test]
fn test_subcommand_help_flag_matches_help_subcommand() {
    let via_flag = padz_cmd().args(["create", "--help"]).output().unwrap();
    let via_sub = padz_cmd().args(["help", "create"]).output().unwrap();

    assert!(via_flag.status.success());
    assert!(via_sub.status.success());
    assert_eq!(
        String::from_utf8_lossy(&via_flag.stdout),
        String::from_utf8_lossy(&via_sub.stdout),
        "`create --help` and `help create` must render identically under Standout 7.6"
    );
}

/// Subcommand help still documents that subcommand's own flags, and exits 0.
#[test]
fn test_subcommand_help_lists_own_flags() {
    padz_cmd()
        .args(["create", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("--no-editor"))
        .stdout(predicate::str::contains("--inside"));
}

/// `-h` behaves like `--help`.
#[test]
fn test_short_help_flag_works() {
    let long = padz_cmd().args(["list", "--help"]).output().unwrap();
    let short = padz_cmd().args(["list", "-h"]).output().unwrap();

    assert_eq!(
        String::from_utf8_lossy(&long.stdout),
        String::from_utf8_lossy(&short.stdout),
        "`-h` and `--help` must render identically"
    );
}

/// Globals stay *accepted* on subcommands even though Standout's help renderer
/// does not list them (see docs/standout-upgrading-feedback.md, "Global args
/// missing from rendered help"). This guards the gap: a documentation gap must
/// not become a parsing regression.
#[test]
fn test_global_flags_still_accepted_on_subcommands() {
    padz_cmd()
        .args(["list", "--global", "--output", "json"])
        .assert()
        .success();
}
