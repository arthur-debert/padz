//! The process-only smoke suite.
//!
//! Every test here names a boundary that cannot be proved faithfully by the
//! in-process harness. Behavior below that boundary belongs to the coverage
//! map in `PROCESS_COVERAGE.md` and to its smallest owning seam.

use assert_cmd::Command;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn padz_bin() -> PathBuf {
    assert_cmd::cargo::cargo_bin!("padz").to_path_buf()
}

struct Fixture {
    _temp: TempDir,
    root: PathBuf,
    project: PathBuf,
    global: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().expect("temporary fixture");
        let root = temp.path().to_path_buf();
        let project = root.join("project");
        let global = root.join("global");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&global).unwrap();
        fs::create_dir(project.join(".git")).unwrap();
        Self {
            _temp: temp,
            root,
            project,
            global,
        }
    }

    fn initialized() -> Self {
        let fixture = Self::new();
        fixture.command().arg("init").assert().success();
        fixture
    }

    fn command(&self) -> Command {
        self.command_at(&self.project)
    }

    fn command_at(&self, cwd: &Path) -> Command {
        let mut command = Command::new(padz_bin());
        command
            .env("PADZ_GLOBAL_DATA", self.global.as_os_str())
            .env("EDITOR", "/usr/bin/false")
            .current_dir(cwd);
        command
    }

    fn create(&self, title: &str) {
        self.command()
            .args(["create", "--no-editor", title])
            .assert()
            .success();
    }
}

fn output_text(output: &[u8]) -> String {
    String::from_utf8(output.to_vec()).expect("process output is UTF-8")
}

/// Process-only boundary: help and topics render and exit during parsing,
/// before a stateful app or handler exists. A child is required both to
/// observe the rendered bytes and to survive the parser's `process::exit`.
#[test]
fn help_render_and_exit_contracts_stay_at_the_process_boundary() {
    let fixture = Fixture::new();

    let root = fixture.command().arg("--help").output().unwrap();
    assert!(root.status.success());
    let root_help = output_text(&root.stdout);
    assert!(root_help.contains("PER PAD(S)"));
    assert!(root_help.contains("LEARN MORE"));
    assert!(root_help.contains("scopes:"));

    let topic = fixture.command().args(["help", "scopes"]).output().unwrap();
    assert!(topic.status.success());
    assert!(output_text(&topic.stdout).contains("PROJECTS AND GLOBAL NOTES"));

    let create_flag = fixture
        .command()
        .args(["create", "--help"])
        .output()
        .unwrap();
    let create_command = fixture.command().args(["help", "create"]).output().unwrap();
    assert!(create_flag.status.success());
    assert!(create_command.status.success());
    assert_eq!(create_flag.stdout, create_command.stdout);
    let create_help = output_text(&create_flag.stdout);
    assert!(create_help.contains("Create a new pad"));
    assert!(create_help.contains("--no-editor"));
    assert!(create_help.contains("--inside"));

    let long = fixture.command().args(["list", "--help"]).output().unwrap();
    let short = fixture.command().args(["list", "-h"]).output().unwrap();
    assert!(long.status.success());
    assert!(short.status.success());
    assert_eq!(long.stdout, short.stdout);

    fixture
        .command()
        .args(["help", "definitely-not-a-topic"])
        .assert()
        .failure();
}

/// Process-only boundary: `config set` is intercepted by clapfig before
/// dispatch. A later process must reload the value that the first process
/// persisted; direct format semantics are covered below the CLI.
#[test]
fn config_set_persists_for_a_later_invocation() {
    let fixture = Fixture::initialized();

    fixture
        .command()
        .args(["config", "set", "format", "md"])
        .assert()
        .success();
    fixture.create("configured markdown");

    let active = fixture.project.join(".padz/active");
    let files: Vec<_> = fs::read_dir(active)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert!(files
        .iter()
        .any(|name| name.to_string_lossy().ends_with(".md")));
}

/// Process-only boundary: independent invocations discover a linked store
/// from their real working directories. Link validation and unlink errors are
/// core/parser facts and therefore live below this suite.
#[test]
fn linked_working_directory_discovers_the_target_store_across_invocations() {
    let fixture = Fixture::new();
    let source = fixture.root.join("source");
    let linked = fixture.root.join("linked");
    for project in [&source, &linked] {
        fs::create_dir_all(project).unwrap();
        fs::create_dir(project.join(".git")).unwrap();
    }

    fixture.command_at(&source).arg("init").assert().success();
    fixture
        .command_at(&source)
        .args(["create", "--no-editor", "from source"])
        .assert()
        .success();
    fixture
        .command_at(&linked)
        .args(["init", "--link", source.to_str().unwrap()])
        .assert()
        .success();
    assert!(linked.join(".padz/link").is_file());

    let listed = fixture
        .command_at(&linked)
        .args(["list", "--output", "json"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let value: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(value["pads"][0]["pad"]["metadata"]["title"], "from source");
}

/// Process-only boundary: the real binary builds two Standout apps. This
/// naked piped invocation proves both install the same invocation resolver and
/// the first parse leaves stdin for the stateful create input chain.
#[test]
fn naked_piped_invocation_uses_the_two_stage_resolver() {
    let fixture = Fixture::initialized();
    let output = fixture
        .command()
        .args(["--output", "json"])
        .write_stdin("Captured\n\nFrom a pipe")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["pads"][0]["pad"]["metadata"]["title"], "Captured");
    assert_eq!(
        value["pads"][0]["pad"]["content"],
        "Captured\n\nFrom a pipe"
    );
}

/// Process-only boundary: a successful human-mode result reaches stdout and
/// leaves stderr clean. Template wording itself remains a harness assertion.
#[test]
fn human_success_reaches_stdout_with_clean_stderr() {
    let fixture = Fixture::initialized();
    fixture.create("human smoke");

    let output = fixture
        .command()
        .args(["pin", "1", "--output", "text"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = output_text(&output.stdout);
    assert!(stdout.contains("Pinned 1 pad..."));
    assert!(stdout.contains("p1. human smoke"));
}

/// Process-only boundary: a message-free WS10 typed result survives final JSON
/// serialization and is written to stdout, not stderr.
#[test]
fn message_free_json_schema_reaches_structured_stdout() {
    let fixture = Fixture::initialized();
    fixture.create("structured smoke");

    let output = fixture
        .command()
        .args(["pin", "1", "--output", "json"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());

    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    let object = value.as_object().expect("structured result object");
    let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["action", "pads", "request"]);
    assert_eq!(value["action"], "pin");
    assert!(object.get("message").is_none());
    assert!(object.get("messages").is_none());
}

/// Process-only boundary: Standout owns final artifact placement. This smoke
/// proves the real binary writes round-trippable bytes to the chosen path, and
/// that a final-write failure is non-zero, uses stderr, and prints no success.
#[test]
fn artifact_destination_round_trips_and_write_failure_is_truthful() {
    let fixture = Fixture::initialized();
    fixture.create("round trip smoke");
    let destination = fixture.root.join("destination");
    fs::create_dir(&destination).unwrap();
    fs::create_dir(destination.join(".git")).unwrap();
    fixture
        .command_at(&destination)
        .arg("init")
        .assert()
        .success();

    let archive = fixture.root.join("chosen.json.tar.gz");
    let exported = fixture
        .command()
        .args([
            "export",
            "--json",
            "--output-file-path",
            archive.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(exported.status.success());
    assert!(exported.stderr.is_empty());
    assert!(archive.is_file());
    assert_eq!(&fs::read(&archive).unwrap()[..2], &[0x1f, 0x8b]);

    fixture
        .command_at(&destination)
        .args(["import", archive.to_str().unwrap()])
        .assert()
        .success();
    let listed = fixture
        .command_at(&destination)
        .args(["list", "--output", "json"])
        .output()
        .unwrap();
    assert!(listed.status.success());
    let value: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(
        value["pads"][0]["pad"]["metadata"]["title"],
        "round trip smoke"
    );

    let failed = fixture
        .command()
        .args([
            "export",
            "--output-file-path",
            fixture.project.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(!failed.stderr.is_empty());
    let combined = format!(
        "{}{}",
        output_text(&failed.stdout),
        output_text(&failed.stderr)
    );
    assert!(!combined.contains("Exported to"));
    assert!(!combined.contains("Exported 1 pad"));
}
