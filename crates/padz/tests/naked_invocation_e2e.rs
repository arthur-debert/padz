//! # Naked invocation — subprocess E2E
//!
//! ## The real boundary this file protects
//!
//! **`cli::run`'s two-stage parse/state wiring, which the in-process app harness
//! does not enter.**
//!
//! Standout now owns the invocation-aware decision in both the initial parsing
//! app and the stateful dispatch app. `tests/harness.rs` covers terminal,
//! piped, piped-empty, globals, and explicit-command precedence at the smaller
//! in-process seam. This single smoke remains to prove the real binary's first
//! parse also installs the resolver before it builds command-specific state.

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
}

/// `cat file | padz` captures the pipe as a new pad.
///
/// Proves both parses resolve the same default and the stateful dispatch reads
/// the still-unconsumed pipe.
#[test]
fn naked_padz_with_a_pipe_creates() {
    let f = Fixture::new();
    let out = f.run_piped(&["--output", "json"], "Captured\n\nFrom a pipe");
    assert_eq!(
        Fixture::pads(&out),
        vec![("Captured".into(), "Captured\n\nFrom a pipe".into())]
    );
}
