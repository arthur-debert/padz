//! # Request-input precedence for `create` / `edit`, and naked invocation
//!
//! These tests pin *where a pad's text comes from*: title args, piped stdin, or
//! the editor — and in which order those win. The rules moved out of the
//! handlers and into `cli::input`'s declarative chain; this suite exists to
//! prove that move changed nothing a user can observe.
//!
//! ## How these tests reach each case
//!
//! Every assertion here runs the real binary, so stdin is a real file
//! descriptor. Two consequences shape the suite:
//!
//! - **`.output()` gives the child a null stdin**, which is *not* a terminal.
//!   Padz therefore treats a plain `padz create` in a test exactly like an
//!   empty pipe — an abort. Tests that want piped content use `write_stdin`;
//!   tests that want the empty-pipe abort simply pass nothing.
//! - **The editor path needs a terminal stdin**, which a spawned process
//!   without a pty cannot have. So the editor arm is *not* reachable here, and
//!   is covered where it can be driven deterministically: the chain's unit
//!   tests in `cli::input`, which inject a `MockStdin::terminal()` reader.
//!
//! That split is deliberate. These tests own the piped/arg precedence through
//! the real binary; the unit tests own the terminal/editor arm through a
//! controlled reader. Neither depends on the developer's editor or clipboard.

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

    fn try_run_piped(&self, args: &[&str], stdin: &str) -> std::process::Output {
        self.cmd()
            .args(args)
            .write_stdin(stdin.to_string())
            .output()
            .unwrap()
    }

    fn set_mode(&self, mode: &str) {
        self.run(&["config", "set", "mode", mode]);
    }

    /// The (title, content) of every pad the command reports, in order.
    ///
    /// `--output json` must precede any title text: `title` is a
    /// `trailing_var_arg`, so anything after it is captured as title words.
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

// ---------------------------------------------------------------------------
// create: the direct/arg path outranks stdin
// ---------------------------------------------------------------------------

/// The precedence's sharpest edge: `--no-editor` uses the args and does **not**
/// read stdin, even with content piped in. Piped text is dropped on this path.
#[test]
fn no_editor_uses_args_and_ignores_piped_stdin() {
    let f = Fixture::new();
    let out = f.run_piped(
        &["create", "--output", "json", "--no-editor", "ArgTitle"],
        "PIPED_CONTENT_IS_IGNORED",
    );
    assert_eq!(
        Fixture::pads(&out),
        vec![("ArgTitle".into(), "ArgTitle".into())]
    );
}

/// `--no-editor` with no title args creates an empty pad — still ignoring stdin.
#[test]
fn no_editor_without_title_creates_an_empty_pad() {
    let f = Fixture::new();
    let out = f.run_piped(&["create", "--output", "json", "--no-editor"], "IGNORED");
    assert_eq!(Fixture::pads(&out), vec![(String::new(), String::new())]);
}

/// Literal `\n` in an argument becomes a real newline, splitting title from body.
#[test]
fn direct_path_expands_literal_newlines() {
    let f = Fixture::new();
    let out = f.run(&[
        "create",
        "--output",
        "json",
        "--no-editor",
        r"Title\nBody line",
    ]);
    assert_eq!(
        Fixture::pads(&out),
        vec![("Title".into(), "Title\n\nBody line".into())]
    );
}

/// Todos mode with title args takes the direct path (editor skipped), so the
/// piped text is ignored exactly as with `--no-editor`.
#[test]
fn todos_mode_with_title_skips_editor_and_ignores_stdin() {
    let f = Fixture::new();
    f.set_mode("todos");
    let out = f.run_piped(&["create", "--output", "json", "Todo Item"], "IGNORED");
    assert_eq!(
        Fixture::pads(&out),
        vec![("Todo Item".into(), "Todo Item".into())]
    );
}

// ---------------------------------------------------------------------------
// create: stdin
// ---------------------------------------------------------------------------

/// Piped stdin with no competing arg source: title and body come from the pipe.
#[test]
fn piped_stdin_supplies_title_and_body() {
    let f = Fixture::new();
    let out = f.run_piped(
        &["create", "--output", "json"],
        "Piped Title\n\nPiped body.",
    );
    assert_eq!(
        Fixture::pads(&out),
        vec![("Piped Title".into(), "Piped Title\n\nPiped body.".into())]
    );
}

/// A title arg overrides the piped buffer's title, keeping the piped body.
#[test]
fn title_arg_overrides_the_piped_title() {
    let f = Fixture::new();
    let out = f.run_piped(
        &["create", "--output", "json", "ArgWins"],
        "StdinTitle\n\nStdinBody",
    );
    assert_eq!(
        Fixture::pads(&out),
        vec![("ArgWins".into(), "ArgWins\n\nStdinBody".into())]
    );
}

/// An empty pipe aborts the create: a warning, and no pad in the store.
///
/// This is the case that makes padz's stdin handling application-owned —
/// standout's own `StdinSource` reports empty input as "no input", which in a
/// chain would fall through to the next source (here, the editor).
#[test]
fn empty_pipe_aborts_and_creates_no_pad() {
    let f = Fixture::new();
    let out = f.run_piped(&["create", "--output", "json"], "");

    assert_eq!(Fixture::pads(&out), vec![]);
    assert!(
        Fixture::messages(&out)
            .iter()
            .any(|m| m.contains("Aborted: empty content")),
        "expected an abort warning, got: {out}"
    );

    let listed = f.run_piped(&["list", "--output", "json"], "");
    assert_eq!(Fixture::pads(&listed), vec![], "the store must stay empty");
}

/// Whitespace-only input is empty too — padz trims before deciding.
#[test]
fn whitespace_only_pipe_aborts() {
    let f = Fixture::new();
    let out = f.run_piped(&["create", "--output", "json"], "   \n  \n");
    assert_eq!(Fixture::pads(&out), vec![]);
    assert!(Fixture::messages(&out)
        .iter()
        .any(|m| m.contains("Aborted: empty content")));
}

/// Nested creation: the piped path still honors `--inside`.
#[test]
fn piped_create_nests_under_inside() {
    let f = Fixture::new();
    f.run(&["create", "--output", "json", "--no-editor", "Parent"]);
    let out = f.run_piped(
        &["create", "--output", "json", "--inside", "1"],
        "ChildTitle\n\nChildBody",
    );

    let v: serde_json::Value = serde_json::from_str(&out).unwrap();
    let parent = &v["pads"][0]["pad"]["metadata"]["parent_id"];
    assert!(
        parent.is_string(),
        "child pad should carry a parent_id, got: {parent}"
    );
    assert_eq!(
        Fixture::pads(&out),
        vec![("ChildTitle".into(), "ChildTitle\n\nChildBody".into())]
    );
}

// ---------------------------------------------------------------------------
// edit
// ---------------------------------------------------------------------------

#[test]
fn edit_takes_content_from_piped_stdin() {
    let f = Fixture::new();
    f.run(&["create", "--output", "json", "--no-editor", "Orig"]);
    let out = f.run_piped(&["edit", "--output", "json", "1"], "NewTitle\n\nNewBody");
    assert_eq!(
        Fixture::pads(&out),
        vec![("NewTitle".into(), "NewTitle\n\nNewBody".into())]
    );
}

/// An empty pipe aborts an edit — and unlike create's warning, this is an error.
#[test]
fn edit_with_empty_pipe_errors() {
    let f = Fixture::new();
    f.run(&["create", "--output", "json", "--no-editor", "Orig"]);
    let out = f.try_run_piped(&["edit", "--output", "json", "1"], "");

    assert!(!out.status.success(), "an empty pipe must fail the edit");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Aborted: empty content"),
        "got stderr: {stderr}"
    );

    // The pad is untouched.
    let listed = f.run_piped(&["list", "--output", "json"], "");
    assert_eq!(Fixture::pads(&listed)[0].0, "Orig");
}

/// Todos mode: trailing words after the index are a quick-edit, beating stdin.
///
/// The words join into one line with no blank-line separator, so the whole
/// thing is the title and the piped text never lands — same shape as the
/// quick-create path.
#[test]
fn todos_edit_uses_inline_words_over_stdin() {
    let f = Fixture::new();
    f.set_mode("todos");
    f.run(&["create", "--output", "json", "T1"]);
    let out = f.run_piped(
        &["edit", "--output", "json", "1", "Edited", "text"],
        "PIPED",
    );
    assert_eq!(
        Fixture::pads(&out),
        vec![("Edited text".into(), "Edited text".into())]
    );
}

/// The quick-edit path expands literal `\n` just like quick-create.
#[test]
fn todos_edit_expands_literal_newlines() {
    let f = Fixture::new();
    f.set_mode("todos");
    f.run(&["create", "--output", "json", "T1"]);
    let out = f.run(&["edit", "--output", "json", "1", r"Edited\nBody"]);
    assert_eq!(
        Fixture::pads(&out),
        vec![("Edited".into(), "Edited\n\nBody".into())]
    );
}

/// `open` shares `edit`'s handler, so it must resolve the same input. Without
/// its own chain registration the handler's input lookup would fail outright.
#[test]
fn open_shares_edits_input_resolution() {
    let f = Fixture::new();
    f.run(&["create", "--output", "json", "--no-editor", "Orig"]);
    let out = f.run_piped(&["open", "--output", "json", "1"], "ViaOpen\n\nBody");
    assert_eq!(
        Fixture::pads(&out),
        vec![("ViaOpen".into(), "ViaOpen\n\nBody".into())]
    );
}

// ---------------------------------------------------------------------------
// naked invocation
// ---------------------------------------------------------------------------

/// `cat file | padz` captures the pipe as a new pad.
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
