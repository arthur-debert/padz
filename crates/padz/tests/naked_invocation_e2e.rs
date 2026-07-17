//! # Naked invocation — subprocess E2E
//!
//! ## The real boundary this file protects
//!
//! **`cli::run`'s own wiring, which no in-process harness can reach.**
//!
//! Which command a bare `padz` means is decided by `cli::run` *before* dispatch:
//! it calls `input::naked_command_from_process()` and injects a synthetic
//! command into the argv it hands to standout. `TestHarness` drives
//! `App::run_to_string`, which starts at dispatch — so it enters *after* the
//! decision this file is about, and cannot test it. Only a real process runs
//! `run()` from the top.
//!
//! `naked_command`'s own logic (terminal → `list`, pipe → `create`, empty pipe →
//! `create`-then-abort) is not what these tests protect: that is covered at a
//! smaller seam by the unit tests in `cli::input`, which inject a `MockStdin`
//! and assert all three arms directly. What is left here, and only here, is that
//! `run()` actually *consults* that decision and injects the command it names —
//! a wiring fact whose only observation point is the process.
//!
//! ## What used to live here
//!
//! This file was `input_precedence_e2e.rs`, and it owned the whole create/edit
//! input-precedence suite. Those cases moved to `tests/harness.rs`, which drives
//! the same input chain in process through an injected stdin reader. The move
//! removed a hole rather than trading one for another: this file's own header
//! used to note that a spawned process has no pty, so a *terminal* stdin — and
//! therefore the editor arm — was unreachable. The harness injects the reader,
//! so it tests both arms; see `a_terminal_stdin_routes_create_to_the_editor`.

use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

/// The built `padz` binary under test.
///
/// The macro form, not the `cargo::cargo_bin(name)` function: the function is
/// deprecated because it breaks under a custom cargo build-dir.
fn padz_bin() -> std::path::PathBuf {
    assert_cmd::cargo::cargo_bin!("padz").to_path_buf()
}

/// A project with an isolated store, so tests never touch the developer's pads.
struct Fixture {
    _temp: TempDir,
    project: std::path::PathBuf,
    global: std::path::PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        let global = temp.path().join("global");
        fs::create_dir_all(&project).unwrap();
        fs::create_dir_all(&global).unwrap();
        // Marks the project root for padz's store discovery.
        fs::create_dir(project.join(".git")).unwrap();

        let fixture = Self {
            _temp: temp,
            project,
            global,
        };
        fixture.run(&["init"]);
        fixture
    }

    fn cmd(&self) -> Command {
        let mut cmd = Command::new(padz_bin());
        cmd.env("PADZ_GLOBAL_DATA", self.global.as_os_str())
            // The editor must never actually launch from a test. If a change
            // ever routes one of these cases to the editor arm, this makes it
            // fail loudly instead of hanging or opening the developer's vim.
            .env("EDITOR", "/bin/false")
            .current_dir(&self.project);
        cmd
    }

    /// Runs padz, asserting success, and returns stdout.
    fn run(&self, args: &[&str]) -> String {
        let out = self.cmd().args(args).output().unwrap();
        assert!(
            out.status.success(),
            "padz {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    /// Runs padz with `stdin` piped in.
    fn run_piped(&self, args: &[&str], stdin: &str) -> String {
        let out = self
            .cmd()
            .args(args)
            .write_stdin(stdin.to_string())
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "padz {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    /// The (title, content) of every pad the command reports, in order.
    fn pads(json: &str) -> Vec<(String, String)> {
        let v: serde_json::Value =
            serde_json::from_str(json).unwrap_or_else(|e| panic!("not JSON: {e}\n{json}"));
        v["pads"]
            .as_array()
            .expect("pads array")
            .iter()
            .map(|p| {
                (
                    p["pad"]["metadata"]["title"].as_str().unwrap().to_string(),
                    p["pad"]["content"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    fn messages(json: &str) -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(json).unwrap();
        v["messages"]
            .as_array()
            .map(|ms| {
                ms.iter()
                    .map(|m| m["content"].as_str().unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// `cat file | padz` captures the pipe as a new pad.
///
/// Proves `run()` injects `create` for a piped naked invocation.
#[test]
fn naked_padz_with_a_pipe_creates() {
    let f = Fixture::new();
    let out = f.run_piped(&["--output", "json"], "Captured\n\nFrom a pipe");
    assert_eq!(
        Fixture::pads(&out),
        vec![("Captured".into(), "Captured\n\nFrom a pipe".into())]
    );
}

/// A naked invocation whose pipe is empty routes to `create`, which then aborts.
/// It must not silently fall back to listing.
#[test]
fn naked_padz_with_an_empty_pipe_aborts_the_create() {
    let f = Fixture::new();
    let out = f.run_piped(&["--output", "json"], "");
    assert!(
        Fixture::messages(&out)
            .iter()
            .any(|m| m.contains("Aborted: empty content")),
        "expected the create abort, got: {out}"
    );
}

/// A naked invocation with no pipe lists.
///
/// `.output()` gives the child a null stdin, which is not a terminal — so padz
/// sees "piped" and this asserts the *create* arm's abort rather than a listing.
/// The terminal arm genuinely needs a pty, and is covered instead by
/// `cli::input`'s unit tests against `MockStdin::terminal()`.
#[test]
fn naked_padz_without_a_terminal_stdin_routes_to_create() {
    let f = Fixture::new();
    f.run(&["create", "--output", "json", "--no-editor", "Existing"]);

    let out = f.run_piped(&["--output", "json"], "");
    assert!(
        Fixture::messages(&out)
            .iter()
            .any(|m| m.contains("Aborted: empty content")),
        "a non-terminal stdin must route to create, not list: {out}"
    );
}
