//! `TestHarness` integration tests — layer 3 of the pyramid (see `src/lib.rs`).
//!
//! # The seam this file protects
//!
//! Everything between argv and rendered output, in process: clap parsing, the
//! declaratively registered pre-dispatch input chains, dispatch to the right
//! handler, the view builders, the templates, the stylesheet, and the
//! output-mode matrix. It is the only
//! layer that can catch a flag wired to the wrong parameter, a template that
//! reads a field the result no longer carries, or a structured mode that
//! accidentally renders human text.
//!
//! # Why every test here is `#[serial]`
//!
//! The seams the harness drives are **process-global**: `$EDITOR` and friends,
//! the working directory, the terminal-width / tty / color detectors, and the
//! default stdin and clipboard readers. Two harness tests running concurrently
//! would install detectors over each other and fail at random. `TestHarness`
//! restores every override when the `TestResult` drops (including on panic), so
//! serial execution is sufficient as well as necessary — and the store itself
//! comes from a fixture value, not from those globals, so nothing leaks between
//! tests through the data.
//!
//! **A harness test without `#[serial]` is a bug**, even if it passes today; the
//! `every_harness_test_is_serial` guard at the bottom of this file enforces that
//! mechanically rather than trusting review to catch it.
//!
//! # Migrated from subprocess
//!
//! The input-precedence cases below came from `input_precedence_e2e.rs`, and the
//! width cases from `terminal_width_e2e.rs`. Both are strictly better off here:
//! a spawned process has no pty, so its stdin can never *be* a terminal, and its
//! terminal width can never be forced — the harness injects both as values. See
//! each section for what its old file could not reach.
//!
//! `create`, `edit`, and `open` are registered explicitly at app assembly so
//! Standout owns their input bags. The direct, piped, empty-pipe, editor, and
//! shared-`open` cases below therefore also guard that composition-root wiring:
//! if a named input is not installed, the handlers' typed lookups fail.

mod support;

use standout::cli::{ExitStatus, OutputKind, RunErrorKind, SuccessKind};
use standout_test::{serial, TestHarness};
use support::Fixture;
use unicode_width::UnicodeWidthStr;

// =============================================================================
// Helpers
// =============================================================================

/// The `(title, content)` of every pad a JSON result reports, in order.
///
/// Structured assertions parse with a real parser rather than `contains`: a
/// substring check passes on human text that merely mentions the word, which is
/// exactly the regression these tests exist to catch.
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

/// Every object key, and every string value, in a parsed structured document —
/// at any nesting depth.
///
/// The distinction the two halves draw is the whole point. A `contains` check on
/// raw output cannot tell a *key* named `indent` from a pad whose *title* merely
/// says "indent": the first is a render-time field leaking into a structured
/// mode, the second is ordinary user data, and they mean opposite things. Parsing
/// separates them, so a leak assertion can name exactly which one it forbids.
///
/// Takes `serde_json::Value` rather than a per-format type so one walker serves
/// every structured mode: YAML deserializes into this same shape, so a caller
/// parses with its own format's real parser and asks the questions here.
#[derive(Default)]
struct Shape {
    keys: std::collections::BTreeSet<String>,
    strings: Vec<String>,
}

fn shape_of(v: &serde_json::Value) -> Shape {
    fn walk(v: &serde_json::Value, shape: &mut Shape) {
        match v {
            serde_json::Value::Object(fields) => {
                for (key, child) in fields {
                    shape.keys.insert(key.clone());
                    walk(child, shape);
                }
            }
            serde_json::Value::Array(items) => items.iter().for_each(|item| walk(item, shape)),
            serde_json::Value::String(text) => shape.strings.push(text.clone()),
            _ => {}
        }
    }

    let mut shape = Shape::default();
    walk(v, &mut shape);
    shape
}

/// The message strings a JSON result carries.
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

// =============================================================================
// Command wiring — argv reaches the right handler with the right arguments
// =============================================================================

#[test]
#[serial]
fn list_dispatches_and_renders_every_pad() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "first pad", "body one");
    fx.seed_pad(&state, "second pad", "body two");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result =
        TestHarness::new()
            .no_color()
            .terminal_width(80)
            .run(&app, cmd, fx.argv(&["list"]));

    result.assert_success();
    result.assert_exit_status(ExitStatus::SUCCESS);
    assert_eq!(result.success_kind(), Some(SuccessKind::Command));
    result.assert_stdout_contains("first pad");
    result.assert_stdout_contains("second pad");
}

#[test]
#[serial]
fn an_unknown_command_does_not_dispatch() {
    let fx = Fixture::new();
    let (app, cmd) = fx.read_app();

    let result =
        TestHarness::new()
            .no_color()
            .run(&app, cmd, fx.argv(&["definitely-not-a-command"]));

    // Clap rejects it before dispatch. Standout 7.7 keeps both the shell status
    // and the failure's origin typed, so this cannot regress to a runtime error.
    result.assert_error();
    result.assert_exit_status(ExitStatus::USAGE_ERROR);
    result.assert_error_kind(RunErrorKind::ClapUsage);
}

// =============================================================================
// Invocation-aware default command
// =============================================================================

#[test]
#[serial]
fn a_naked_invocation_lists_when_stdin_is_a_terminal() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "existing", "body");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result =
        TestHarness::new()
            .interactive_stdin()
            .run(&app, cmd, fx.argv(&["--output", "json"]));

    result.assert_success();
    assert_eq!(
        pads(result.stdout()),
        vec![("existing".into(), "existing\n\nbody".into())]
    );
}

#[test]
#[serial]
fn a_naked_invocation_creates_from_piped_stdin() {
    let fx = Fixture::new();
    let (app, cmd) = fx.read_app();
    let result = TestHarness::new()
        .piped_stdin("Captured\n\nFrom a pipe")
        .run(&app, cmd, fx.argv(&["--output", "json"]));

    result.assert_success();
    assert_eq!(
        pads(result.stdout()),
        vec![("Captured".into(), "Captured\n\nFrom a pipe".into())]
    );
}

#[test]
#[serial]
fn a_naked_invocation_with_an_empty_pipe_aborts_create() {
    let fx = Fixture::new();
    let (app, cmd) = fx.read_app();
    let result = TestHarness::new()
        .piped_stdin("")
        .run(&app, cmd, fx.argv(&["--output", "json"]));

    result.assert_success();
    assert_eq!(pads(result.stdout()), Vec::<(String, String)>::new());
    assert!(messages(result.stdout())
        .iter()
        .any(|message| message.contains("Aborted: empty content")));
}

#[test]
#[serial]
fn an_explicit_command_keeps_precedence_over_piped_stdin() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "existing", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().piped_stdin("must not create").run(
        &app,
        cmd,
        fx.argv(&["list", "--output", "json"]),
    );

    result.assert_success();
    assert_eq!(
        pads(result.stdout()),
        vec![("existing".into(), "existing".into())]
    );
}

#[test]
#[serial]
fn search_flag_reaches_the_handler_as_a_filter() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "shopping list", "");
    fx.seed_pad(&state, "meeting notes", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().run(
        &app,
        cmd,
        fx.argv(&["search", "meeting", "--output", "json"]),
    );

    let got: Vec<String> = pads(result.stdout()).into_iter().map(|(t, _)| t).collect();
    assert_eq!(got, vec!["meeting notes"]);
}

// =============================================================================
// Semantic initialization and maintenance outcomes
// =============================================================================

#[test]
#[serial]
fn initialization_preserves_text_and_exposes_structured_facts() {
    let fx = Fixture::new();
    let (app, cmd) = fx.app(&["init"]);
    let text = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        fx.argv(&["init", "--output", "text"]),
    );

    text.assert_success();
    assert_eq!(
        text.stdout(),
        format!(
            "Initialized padz store at {}\n\nTip: Enable shell completions for padz:\n  \
             eval \"$(padz completions bash)\"  # add to ~/.bashrc\n  \
             eval \"$(padz completions zsh)\"   # add to ~/.zshrc\n",
            fx.project().join(".padz").display()
        )
    );
    drop(text);

    let json = TestHarness::new().run(&app, cmd, fx.argv(&["init", "--output", "json"]));
    json.assert_success();
    let mut value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    value["store_path"] = serde_json::json!("<STORE_PATH>");
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/initialization.json"
    ))
    .unwrap();
    assert_eq!(value, fixture);
    assert!(
        value.get("messages").is_none(),
        "initialization schema must expose facts rather than prose"
    );
}

#[test]
#[serial]
fn doctor_preserves_clean_text_and_exposes_counts() {
    let fx = Fixture::new();
    let (app, cmd) = fx.read_app();
    let text = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        fx.argv(&["doctor", "--output", "text"]),
    );

    text.assert_success();
    assert_eq!(text.stdout(), "No inconsistencies found.\n");
    drop(text);

    let json = TestHarness::new().run(&app, cmd, fx.argv(&["doctor", "--output", "json"]));
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/semantic_outcomes/doctor-clean.json")).unwrap();
    assert_eq!(value, fixture);
}

#[test]
#[serial]
fn purge_preserves_completed_text_and_exposes_selection_and_counts() {
    let text_fx = Fixture::new();
    let state = text_fx.app_state();
    text_fx.seed_pad(&state, "gone", "");
    state
        .with_api(|api| api.delete_pads(state.scope, &["1"]))
        .unwrap();
    drop(state);
    let (app, cmd) = text_fx.read_app();
    let text = TestHarness::new().no_color().run(
        &app,
        cmd,
        text_fx.argv(&["purge", "--yes", "--output", "text"]),
    );

    text.assert_success();
    assert_eq!(text.stdout(), "Purging 1 pad(s)...\nPurged: d1 gone\n");

    let json_fx = Fixture::new();
    let state = json_fx.app_state();
    json_fx.seed_pad(&state, "gone", "");
    state
        .with_api(|api| api.delete_pads(state.scope, &["1"]))
        .unwrap();
    drop(state);
    let (app, cmd) = json_fx.read_app();
    let json = TestHarness::new().run(
        &app,
        cmd,
        json_fx.argv(&["purge", "--yes", "--output", "json"]),
    );

    json.assert_success();
    let mut value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    assert!(value["selected_pads"][0]["id"]
        .as_str()
        .is_some_and(|id| id.len() == 36));
    value["selected_pads"][0]["id"] = serde_json::json!("<UUID>");
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/purge-completed.json"
    ))
    .unwrap();
    assert_eq!(value, fixture);
}

#[test]
#[serial]
fn empty_purge_is_distinct_in_text_and_structured_output() {
    let fx = Fixture::new();
    let (app, cmd) = fx.read_app();
    let text = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        fx.argv(&["purge", "--yes", "--output", "text"]),
    );
    text.assert_success();
    assert_eq!(text.stdout(), "No pads to purge.\n");
    drop(text);

    let json = TestHarness::new().run(&app, cmd, fx.argv(&["purge", "--yes", "--output", "json"]));
    json.assert_success();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/semantic_outcomes/purge-empty.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(json.stdout()).unwrap(),
        fixture
    );
}

#[test]
#[serial]
fn import_preserves_warning_text_and_exposes_partial_success_facts() {
    let text_fx = Fixture::new();
    let source = text_fx.root().join("bad.md");
    std::fs::write(
        &source,
        "---\npadz.status: NotAThing\n---\n\nImported title\n\nBody",
    )
    .unwrap();
    let source_text = source.display().to_string();
    let (app, cmd) = text_fx.app(&["import", &source_text]);
    let text = TestHarness::new().no_color().run(
        &app,
        cmd,
        text_fx.argv(&["import", &source_text, "--output", "text"]),
    );
    text.assert_success();
    assert_eq!(
        text.stdout(),
        format!(
            "Imported: {source_text}\n{source_text}: applied inline metadata\n\
             {source_text}: invalid status\nTotal imported: 1\n"
        )
    );
    drop(text);

    let json_fx = Fixture::new();
    let source = json_fx.root().join("bad.md");
    std::fs::write(
        &source,
        "---\npadz.status: NotAThing\n---\n\nImported title\n\nBody",
    )
    .unwrap();
    let source_text = source.display().to_string();
    let (app, cmd) = json_fx.app(&["import", &source_text]);
    let json = TestHarness::new().run(
        &app,
        cmd,
        json_fx.argv(&["import", &source_text, "--output", "json"]),
    );
    json.assert_success();
    let mut value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    value["sources"][0]["source"] = serde_json::json!("<SOURCE>");
    value["sources"][0]["processed_files"][0] = serde_json::json!("<SOURCE>");
    value["sources"][0]["diagnostics"][0]["source_label"] = serde_json::json!("<SOURCE>");
    value["sources"][0]["diagnostics"][1]["warning"]["source_label"] =
        serde_json::json!("<SOURCE>");
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/import-partial.json"
    ))
    .unwrap();
    assert_eq!(value, fixture);
    assert!(value.get("messages").is_none());
}

#[test]
#[serial]
fn import_report_serializes_in_every_structured_mode() {
    for mode in ["json", "yaml", "xml", "csv"] {
        let fx = Fixture::new();
        let source = fx.root().join("plain.md");
        std::fs::write(&source, "Imported title\n\nBody").unwrap();
        let source = source.display().to_string();
        let (app, cmd) = fx.app(&["import", &source]);
        let result =
            TestHarness::new().run(&app, cmd, fx.argv(&["import", &source, "--output", mode]));
        result.assert_success();
        match mode {
            "json" => {
                serde_json::from_str::<serde_json::Value>(result.stdout()).unwrap();
            }
            "yaml" => {
                serde_yaml::from_str::<serde_yaml::Value>(result.stdout()).unwrap();
            }
            "xml" => {
                let mut reader = quick_xml::Reader::from_str(result.stdout());
                loop {
                    match reader.read_event() {
                        Ok(quick_xml::events::Event::Eof) => break,
                        Ok(_) => {}
                        Err(error) => panic!("invalid import XML: {error}\n{}", result.stdout()),
                    }
                }
            }
            "csv" => {
                let mut reader = csv::Reader::from_reader(result.stdout().as_bytes());
                reader.headers().unwrap();
                for row in reader.records() {
                    row.unwrap();
                }
            }
            _ => unreachable!(),
        }
    }
}

// =============================================================================
// Semantic cross-store transfer outcomes
// =============================================================================

#[test]
#[serial]
fn clone_preserves_text_and_exposes_transfer_facts() {
    let text_source = Fixture::new();
    let text_peer = Fixture::new();
    let state = text_source.app_state();
    text_source.seed_pad(&state, "Alpha", "body");
    drop(state);
    let peer_arg = text_peer.project().display().to_string();
    let peer_store = text_peer
        .project()
        .join(".padz")
        .canonicalize()
        .unwrap()
        .display()
        .to_string();
    let (app, cmd) = text_source.app(&["clone", "1", "--to", &peer_arg]);
    let text = TestHarness::new().no_color().run(
        &app,
        cmd,
        text_source.argv(&["clone", "1", "--to", &peer_arg, "--output", "text"]),
    );
    text.assert_success();
    assert_eq!(text.stdout(), format!("Cloned 1 pad(s) to {peer_store}\n"));
    drop(text);

    let (app, cmd) = text_source.app(&["clone", "1", "--to", &peer_arg]);
    let styled = TestHarness::new().run(
        &app,
        cmd,
        text_source.argv(&["clone", "1", "--to", &peer_arg, "--output", "term-debug"]),
    );
    styled.assert_success();
    assert_eq!(
        styled.stdout(),
        format!("[success]Cloned 1 pad(s) to {peer_store}[/success]\n")
    );
    drop(styled);

    let json_source = Fixture::new();
    let json_peer = Fixture::new();
    let state = json_source.app_state();
    json_source.seed_pad(&state, "Alpha", "body");
    drop(state);
    let peer_arg = json_peer.project().display().to_string();
    let (app, cmd) = json_source.app(&["clone", "1", "--to", &peer_arg]);
    let json = TestHarness::new().run(
        &app,
        cmd,
        json_source.argv(&["clone", "1", "--to", &peer_arg, "--output", "json"]),
    );
    json.assert_success();
    let mut value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    value["peer_store"] = serde_json::json!("<PEER_STORE>");
    value["copied_pad_ids"][0] = serde_json::json!("<PAD_ID>");
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/clone-success.json"
    ))
    .unwrap();
    assert_eq!(value, expected);
    assert!(value.get("messages").is_none());
}

#[test]
#[serial]
fn clone_empty_and_parent_orphaning_are_explicit_states() {
    let empty_source = Fixture::new();
    let empty_peer = Fixture::new();
    let peer_arg = empty_peer.project().display().to_string();
    let (app, cmd) = empty_source.app(&["clone", "--to", &peer_arg]);
    let empty = TestHarness::new().run(
        &app,
        cmd,
        empty_source.argv(&["clone", "--to", &peer_arg, "--output", "json"]),
    );
    empty.assert_success();
    let empty_value: serde_json::Value = serde_json::from_str(empty.stdout()).unwrap();
    assert_eq!(empty_value["status"], "empty");
    assert_eq!(
        empty_value["requested_selection"]["kind"],
        "all_non_deleted"
    );
    assert_eq!(empty_value["copied_count"], 0);
    assert_eq!(empty_value["diagnostics"], serde_json::json!([]));
    drop(empty);

    let source = Fixture::new();
    let peer = Fixture::new();
    let state = source.app_state();
    source.seed_pad(&state, "Parent", "");
    source.seed_child(&state, "1", "Child", "");
    drop(state);
    let peer_arg = peer.project().display().to_string();
    let (app, cmd) = source.app(&["clone", "1.1", "--to", &peer_arg]);
    let orphaned = TestHarness::new().run(
        &app,
        cmd,
        source.argv(&["clone", "1.1", "--to", &peer_arg, "--output", "json"]),
    );
    orphaned.assert_success();
    let value: serde_json::Value = serde_json::from_str(orphaned.stdout()).unwrap();
    assert_eq!(value["status"], "partial_success");
    assert_eq!(value["copied_count"], 1);
    assert_eq!(value["diagnostics"][0]["kind"], "parent_orphaned");
    assert_eq!(
        value["diagnostics"][0]["pad_id"],
        value["copied_pad_ids"][0]
    );
    assert!(value["diagnostics"][0]["parent_id"].is_string());
    drop(orphaned);

    let styled_peer = Fixture::new();
    let styled_peer_arg = styled_peer.project().display().to_string();
    let (app, cmd) = source.app(&["clone", "1.1", "--to", &styled_peer_arg]);
    let styled = TestHarness::new().run(
        &app,
        cmd,
        source.argv(&[
            "clone",
            "1.1",
            "--to",
            &styled_peer_arg,
            "--output",
            "term-debug",
        ]),
    );
    styled.assert_success();
    let orphan_position = styled
        .stdout()
        .find("[info]Pad ")
        .expect("orphan info style");
    let summary_position = styled
        .stdout()
        .find("[success]Cloned 1 pad(s)")
        .expect("success summary style");
    assert!(orphan_position < summary_position);
    assert!(styled
        .stdout()
        .contains("parent not in move set, orphaned to root[/info]"));
}

#[test]
#[serial]
fn transfer_report_serializes_in_every_structured_mode() {
    for mode in ["json", "yaml", "xml", "csv"] {
        let source = Fixture::new();
        let peer = Fixture::new();
        let state = source.app_state();
        source.seed_pad(&state, "Alpha", "body");
        drop(state);
        let peer_arg = peer.project().display().to_string();
        let (app, cmd) = source.app(&["clone", "1", "--to", &peer_arg]);
        let result = TestHarness::new().run(
            &app,
            cmd,
            source.argv(&["clone", "1", "--to", &peer_arg, "--output", mode]),
        );
        result.assert_success();
        match mode {
            "json" => {
                serde_json::from_str::<serde_json::Value>(result.stdout()).unwrap();
            }
            "yaml" => {
                serde_yaml::from_str::<serde_yaml::Value>(result.stdout()).unwrap();
            }
            "xml" => {
                let mut reader = quick_xml::Reader::from_str(result.stdout());
                loop {
                    match reader.read_event() {
                        Ok(quick_xml::events::Event::Eof) => break,
                        Ok(_) => {}
                        Err(error) => {
                            panic!("invalid transfer XML: {error}\n{}", result.stdout())
                        }
                    }
                }
            }
            "csv" => {
                let mut reader = csv::Reader::from_reader(result.stdout().as_bytes());
                reader.headers().unwrap();
                for row in reader.records() {
                    row.unwrap();
                }
            }
            _ => unreachable!(),
        }
    }
}

// =============================================================================
// Semantic pad mutation outcomes
// =============================================================================

#[test]
#[serial]
fn nested_edit_preserves_text_and_exposes_update_facts() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "parent", "");
    fx.seed_child(&state, "1", "child", "");
    drop(state);
    let (app, cmd) = fx.app(&["list"]);

    let text = TestHarness::new()
        .no_color()
        .piped_stdin("Edited child")
        .run(
            &app,
            cmd.clone(),
            fx.argv(&["edit", "1.1", "--output", "text"]),
        );
    text.assert_success();
    text.assert_stdout_contains("Updated 1 pad...");
    text.assert_stdout_contains("Updated (1.1): Edited child");
    drop(text);

    let json = TestHarness::new().piped_stdin("Edited child").run(
        &app,
        cmd,
        fx.argv(&["edit", "1.1", "--output", "json"]),
    );
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/semantic_outcomes/edit-content.json")).unwrap();
    assert_eq!(value["outcomes"], fixture);
    assert!(value["messages"].as_array().is_some_and(Vec::is_empty));
}

#[test]
#[serial]
fn same_parent_move_preserves_text_and_exposes_the_nested_no_op() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "parent", "");
    fx.seed_child(&state, "1", "child", "");
    drop(state);
    let (app, cmd) = fx.read_app();

    let text = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        fx.argv(&["move", "1.1", "1", "--output", "text"]),
    );
    text.assert_success();
    assert_eq!(text.stdout(), "Pad '1.1' is already at destination\n");
    drop(text);

    let json = TestHarness::new().run(
        &app,
        cmd,
        fx.argv(&["move", "1.1", "1", "--output", "json"]),
    );
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/semantic_outcomes/move-no-op.json")).unwrap();
    assert_eq!(value["notices"], fixture);
    assert!(value["messages"].as_array().is_some_and(Vec::is_empty));
}

#[test]
#[serial]
fn mixed_complete_distinguishes_changed_pads_from_no_ops() {
    let fx = Fixture::new();
    fx.todos_mode();
    let state = fx.app_state();
    fx.seed_pad(&state, "changed", "");
    fx.seed_pad(&state, "no op", "");
    state
        .with_api(|api| api.complete_pads(state.scope, &["1"]))
        .unwrap();
    drop(state);
    let (app, cmd) = fx.read_app();

    let json = TestHarness::new().run(
        &app,
        cmd,
        fx.argv(&["complete", "1", "2", "--output", "json"]),
    );
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/complete-mixed.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::json!({
            "notices": value["notices"].clone(),
            "outcomes": value["outcomes"].clone(),
        }),
        fixture
    );
    assert_eq!(value["pads"].as_array().map(Vec::len), Some(2));
    assert!(value["pads"]
        .as_array()
        .unwrap()
        .iter()
        .all(|pad| pad["pad"]["metadata"]["status"] == "Done"));
}

#[test]
#[serial]
fn repeated_complete_and_reopen_keep_compatible_text_and_typed_statuses() {
    let complete_fx = Fixture::new();
    complete_fx.todos_mode();
    let state = complete_fx.app_state();
    complete_fx.seed_pad(&state, "done", "");
    state
        .with_api(|api| api.complete_pads(state.scope, &["1"]))
        .unwrap();
    drop(state);
    let (app, cmd) = complete_fx.read_app();
    let complete = TestHarness::new().no_color().run(
        &app,
        cmd,
        complete_fx.argv(&["complete", "1", "--output", "text"]),
    );
    complete.assert_success();
    complete.assert_stdout_contains("Pad 1 is already done");
    drop(complete);

    let reopen_fx = Fixture::new();
    reopen_fx.todos_mode();
    let state = reopen_fx.app_state();
    reopen_fx.seed_pad(&state, "planned", "");
    drop(state);
    let (app, cmd) = reopen_fx.read_app();
    let text = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        reopen_fx.argv(&["reopen", "1", "--output", "text"]),
    );
    text.assert_success();
    text.assert_stdout_contains("Pad 1 is already planned");
    drop(text);

    let json = TestHarness::new().run(
        &app,
        cmd,
        reopen_fx.argv(&["reopen", "1", "--output", "json"]),
    );
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/semantic_outcomes/reopen-no-op.json")).unwrap();
    assert_eq!(value["notices"], fixture);
}

#[test]
#[serial]
fn empty_delete_completed_preserves_text_and_exposes_a_typed_no_op() {
    let fx = Fixture::new();
    fx.todos_mode();
    let state = fx.app_state();
    fx.seed_pad(&state, "still open", "");
    drop(state);
    let (app, cmd) = fx.read_app();

    let text = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        fx.argv(&["delete", "--completed", "--output", "text"]),
    );
    text.assert_success();
    assert_eq!(text.stdout(), "No completed pads to delete.\n");
    drop(text);

    let json = TestHarness::new().run(
        &app,
        cmd,
        fx.argv(&["delete", "--completed", "--output", "json"]),
    );
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/delete-completed-empty.json"
    ))
    .unwrap();
    assert_eq!(value["notices"], fixture);
    assert!(value["messages"].as_array().is_some_and(Vec::is_empty));
}

// =============================================================================
// Semantic tag catalog and mutation outcomes
// =============================================================================

#[test]
#[serial]
fn tag_catalog_preserves_empty_and_ordered_human_and_structured_results() {
    let empty_fx = Fixture::new();
    let (app, cmd) = empty_fx.read_app();
    let text = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        empty_fx.argv(&["tag", "list", "--output", "text"]),
    );
    text.assert_success();
    assert_eq!(text.stdout(), "No tags defined\n");
    drop(text);

    let json = TestHarness::new().run(
        &app,
        cmd,
        empty_fx.argv(&["tag", "list", "--output", "json"]),
    );
    json.assert_success();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/tag-catalog-empty.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(json.stdout()).unwrap(),
        expected
    );
    drop(json);

    let listed_fx = Fixture::new();
    let state = listed_fx.app_state();
    state
        .with_api(|api| api.create_tag(state.scope, "work"))
        .unwrap();
    state
        .with_api(|api| api.create_tag(state.scope, "rust"))
        .unwrap();
    drop(state);
    let (app, cmd) = listed_fx.read_app();
    let debug = TestHarness::new().run(
        &app,
        cmd.clone(),
        listed_fx.argv(&["tag", "list", "--output", "term-debug"]),
    );
    debug.assert_success();
    assert_eq!(debug.stdout(), "[info]work[/info]\n[info]rust[/info]\n");
    drop(debug);

    let json = TestHarness::new().run(
        &app,
        cmd,
        listed_fx.argv(&["tag", "list", "--output", "json"]),
    );
    json.assert_success();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/tag-catalog-listed.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(json.stdout()).unwrap(),
        expected
    );
    drop(json);

    let singleton_fx = Fixture::new();
    let state = singleton_fx.app_state();
    singleton_fx.seed_pad(&state, "target", "");
    state
        .with_api(|api| api.add_tags_to_pads(state.scope, &["1"], &["work".into()]))
        .unwrap();
    drop(state);
    let (app, cmd) = singleton_fx.read_app();
    let text = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        singleton_fx.argv(&["tag", "list", "1", "--output", "text"]),
    );
    text.assert_success();
    assert_eq!(text.stdout(), "work\n");
    drop(text);

    let json = TestHarness::new().run(
        &app,
        cmd,
        singleton_fx.argv(&["tag", "list", "1", "--output", "json"]),
    );
    json.assert_success();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/tag-catalog-singleton.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(json.stdout()).unwrap(),
        expected
    );
}

#[test]
#[serial]
fn tag_assignment_preserves_wording_styles_counts_and_no_op_kind() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "one", "");
    fx.seed_pad(&state, "two", "");
    drop(state);
    let (app, cmd) = fx.read_app();

    let debug = TestHarness::new().run(
        &app,
        cmd.clone(),
        fx.argv(&[
            "tag",
            "add",
            "1",
            "2",
            "work",
            "rust",
            "--output",
            "term-debug",
        ]),
    );
    debug.assert_success();
    debug.assert_stdout_contains("[info]Tagged 2 pads...[/info]");
    debug.assert_stdout_contains("[success]Added tags [work, rust] to 2 pads[/success]");
    drop(debug);

    let no_op = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        fx.argv(&["tag", "add", "1", "2", "work", "rust", "--output", "text"]),
    );
    no_op.assert_success();
    no_op.assert_stdout_contains("Tagged 2 pads...");
    no_op.assert_stdout_contains("All pads already have tags [work, rust]");
    drop(no_op);

    let json = TestHarness::new().run(
        &app,
        cmd,
        fx.argv(&["tag", "add", "1", "2", "work", "rust", "--output", "json"]),
    );
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/tag-all-already-present.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::json!({
            "action": value["action"].clone(),
            "requested_tags": value["requested_tags"].clone(),
            "modified_pads": value["modified_pads"].clone(),
        }),
        expected
    );
    assert_eq!(value["pads"].as_array().map(Vec::len), Some(2));
    drop(json);

    let changed_fx = Fixture::new();
    let state = changed_fx.app_state();
    changed_fx.seed_pad(&state, "one", "");
    changed_fx.seed_pad(&state, "two", "");
    drop(state);
    let (app, cmd) = changed_fx.read_app();
    let json = TestHarness::new().run(
        &app,
        cmd,
        changed_fx.argv(&["tag", "add", "1", "2", "work", "rust", "--output", "json"]),
    );
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/semantic_outcomes/tag-assigned.json")).unwrap();
    assert_eq!(
        serde_json::json!({
            "action": value["action"].clone(),
            "requested_tags": value["requested_tags"].clone(),
            "modified_pads": value["modified_pads"].clone(),
        }),
        expected
    );
}

#[test]
#[serial]
fn tag_removal_preserves_wording_counts_and_none_present_no_op() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    state
        .with_api(|api| api.add_tags_to_pads(state.scope, &["1"], &["work".into(), "rust".into()]))
        .unwrap();
    drop(state);
    let (app, cmd) = fx.read_app();

    let text = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        fx.argv(&["tag", "remove", "1", "work", "rust", "--output", "text"]),
    );
    text.assert_success();
    text.assert_stdout_contains("Untagged 1 pad...");
    text.assert_stdout_contains("Removed tags [work, rust] from 1 pad");
    drop(text);

    let no_op = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        fx.argv(&["tag", "remove", "1", "work", "rust", "--output", "text"]),
    );
    no_op.assert_success();
    no_op.assert_stdout_contains("No pads had tags [work, rust]");
    drop(no_op);

    let json = TestHarness::new().run(
        &app,
        cmd,
        fx.argv(&["tag", "remove", "1", "work", "rust", "--output", "json"]),
    );
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let expected: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/semantic_outcomes/tag-none-present.json"
    ))
    .unwrap();
    assert_eq!(
        serde_json::json!({
            "action": value["action"].clone(),
            "requested_tags": value["requested_tags"].clone(),
            "modified_pads": value["modified_pads"].clone(),
        }),
        expected
    );
    drop(json);

    let changed_fx = Fixture::new();
    let state = changed_fx.app_state();
    changed_fx.seed_pad(&state, "target", "");
    state
        .with_api(|api| api.add_tags_to_pads(state.scope, &["1"], &["work".into(), "rust".into()]))
        .unwrap();
    drop(state);
    let (app, cmd) = changed_fx.read_app();
    let json = TestHarness::new().run(
        &app,
        cmd,
        changed_fx.argv(&["tag", "remove", "1", "work", "rust", "--output", "json"]),
    );
    json.assert_success();
    let value: serde_json::Value = serde_json::from_str(json.stdout()).unwrap();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/semantic_outcomes/tag-removed.json")).unwrap();
    assert_eq!(
        serde_json::json!({
            "action": value["action"].clone(),
            "requested_tags": value["requested_tags"].clone(),
            "modified_pads": value["modified_pads"].clone(),
        }),
        expected
    );
}

#[test]
#[serial]
fn tag_registry_mutations_preserve_names_counts_and_human_wording() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    state
        .with_api(|api| api.add_tags_to_pads(state.scope, &["1"], &["old".into()]))
        .unwrap();
    drop(state);
    let (app, cmd) = fx.read_app();

    let rename = TestHarness::new().no_color().run(
        &app,
        cmd.clone(),
        fx.argv(&["tag", "rename", "old", "new", "--output", "text"]),
    );
    rename.assert_success();
    assert_eq!(
        rename.stdout(),
        "Renamed tag 'old' to 'new'\nUpdated 1 pad\n"
    );
    drop(rename);

    let delete = TestHarness::new().no_color().run(
        &app,
        cmd,
        fx.argv(&["tag", "delete", "new", "--output", "text"]),
    );
    delete.assert_success();
    assert_eq!(delete.stdout(), "Deleted tag 'new'\nRemoved from 1 pad\n");
    drop(delete);

    let structured_fx = Fixture::new();
    let state = structured_fx.app_state();
    structured_fx.seed_pad(&state, "target", "");
    state
        .with_api(|api| api.add_tags_to_pads(state.scope, &["1"], &["old".into()]))
        .unwrap();
    drop(state);
    let (app, cmd) = structured_fx.read_app();

    let rename = TestHarness::new().run(
        &app,
        cmd.clone(),
        structured_fx.argv(&["tag", "rename", "old", "new", "--output", "json"]),
    );
    rename.assert_success();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/semantic_outcomes/tag-renamed.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(rename.stdout()).unwrap(),
        expected
    );
    drop(rename);

    let delete = TestHarness::new().run(
        &app,
        cmd,
        structured_fx.argv(&["tag", "delete", "new", "--output", "json"]),
    );
    delete.assert_success();
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/semantic_outcomes/tag-deleted.json")).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(delete.stdout()).unwrap(),
        expected
    );
}

#[test]
#[serial]
fn uuid_flag_reaches_the_view_handler() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "recipe", "mix and bake");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().run(
        &app,
        cmd,
        fx.argv(&["view", "1", "--uuid", "--output", "json"]),
    );

    let v: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert!(
        v["pads"][0]["uuid"].is_string(),
        "--uuid must reach the handler and put a uuid on the result: {}",
        result.stdout()
    );
}

// =============================================================================
// Input chain — the pre-dispatch hooks, driven through real argv and stdin
// =============================================================================
//
// Migrated from `input_precedence_e2e.rs`. That file drove these through a
// spawned binary and documented the resulting hole: `.output()` gives the child
// a null stdin, so a *terminal* stdin was unreachable and the editor arm could
// not be tested at all. Here stdin is an injected reader, so both arms are
// reachable from one place — see `a_terminal_stdin_routes_create_to_the_editor`.

#[test]
#[serial]
fn no_editor_uses_args_and_ignores_piped_stdin() {
    let fx = Fixture::new();
    let (app, cmd) = fx.app(&["create"]);

    let result = TestHarness::new()
        .no_color()
        .piped_stdin("PIPED_CONTENT_IS_IGNORED")
        .run(
            &app,
            cmd,
            fx.argv(&["create", "--output", "json", "--no-editor", "ArgTitle"]),
        );

    assert_eq!(
        pads(result.stdout()),
        vec![("ArgTitle".to_string(), "ArgTitle".to_string())],
        "the direct path must not read stdin at all"
    );
}

#[test]
#[serial]
fn no_editor_without_title_creates_an_empty_pad() {
    let fx = Fixture::new();
    let (app, cmd) = fx.app(&["create"]);

    let result = TestHarness::new().no_color().piped_stdin("IGNORED").run(
        &app,
        cmd,
        fx.argv(&["create", "--output", "json", "--no-editor"]),
    );

    assert_eq!(pads(result.stdout()), vec![(String::new(), String::new())]);
}

#[test]
#[serial]
fn direct_path_expands_literal_newlines() {
    let fx = Fixture::new();
    let (app, cmd) = fx.app(&["create"]);

    let result = TestHarness::new().no_color().interactive_stdin().run(
        &app,
        cmd,
        fx.argv(&[
            "create",
            "--output",
            "json",
            "--no-editor",
            r"Title\nBody line",
        ]),
    );

    assert_eq!(
        pads(result.stdout()),
        vec![("Title".to_string(), "Title\n\nBody line".to_string())]
    );
}

#[test]
#[serial]
fn piped_stdin_supplies_title_and_body() {
    let fx = Fixture::new();
    let (app, cmd) = fx.app(&["create"]);

    let result = TestHarness::new()
        .no_color()
        .piped_stdin("Piped Title\n\nPiped body.")
        .run(&app, cmd, fx.argv(&["create", "--output", "json"]));

    assert_eq!(
        pads(result.stdout()),
        vec![(
            "Piped Title".to_string(),
            "Piped Title\n\nPiped body.".to_string()
        )]
    );
}

#[test]
#[serial]
fn title_arg_overrides_the_piped_title() {
    let fx = Fixture::new();
    let (app, cmd) = fx.app(&["create"]);

    let result = TestHarness::new()
        .no_color()
        .piped_stdin("StdinTitle\n\nStdinBody")
        .run(
            &app,
            cmd,
            fx.argv(&["create", "--output", "json", "ArgWins"]),
        );

    assert_eq!(
        pads(result.stdout()),
        vec![("ArgWins".to_string(), "ArgWins\n\nStdinBody".to_string())]
    );
}

#[test]
#[serial]
fn empty_pipe_aborts_and_creates_no_pad() {
    let fx = Fixture::new();
    let (app, cmd) = fx.app(&["create"]);

    let result = TestHarness::new().no_color().piped_stdin("").run(
        &app,
        cmd,
        fx.argv(&["create", "--output", "json"]),
    );

    assert_eq!(pads(result.stdout()), vec![]);
    assert!(
        messages(result.stdout())
            .iter()
            .any(|m| m.contains("Aborted: empty content")),
        "expected an abort warning, got: {}",
        result.stdout()
    );
    drop(result);

    // The store must stay empty — the abort is not a create that rendered oddly.
    let (app, cmd) = fx.read_app();
    let listed =
        TestHarness::new()
            .no_color()
            .run(&app, cmd, fx.argv(&["list", "--output", "json"]));
    assert_eq!(pads(listed.stdout()), vec![]);
}

#[test]
#[serial]
fn whitespace_only_pipe_aborts() {
    let fx = Fixture::new();
    let (app, cmd) = fx.app(&["create"]);

    let result = TestHarness::new().no_color().piped_stdin("   \n  \n").run(
        &app,
        cmd,
        fx.argv(&["create", "--output", "json"]),
    );

    assert_eq!(pads(result.stdout()), vec![]);
    assert!(messages(result.stdout())
        .iter()
        .any(|m| m.contains("Aborted: empty content")));
}

#[test]
#[serial]
fn piped_create_nests_under_inside() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "Parent", "");
    drop(state);

    let (app, cmd) = fx.app(&["create"]);
    let result = TestHarness::new()
        .no_color()
        .piped_stdin("ChildTitle\n\nChildBody")
        .run(
            &app,
            cmd,
            fx.argv(&["create", "--output", "json", "--inside", "1"]),
        );

    let v: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();
    assert!(
        v["pads"][0]["pad"]["metadata"]["parent_id"].is_string(),
        "the piped path must still honor --inside: {}",
        result.stdout()
    );
}

#[test]
#[serial]
fn todos_mode_with_title_skips_editor_and_ignores_stdin() {
    let fx = Fixture::new();
    fx.todos_mode();
    let (app, cmd) = fx.app(&["create"]);

    let result = TestHarness::new().no_color().piped_stdin("IGNORED").run(
        &app,
        cmd,
        fx.argv(&["create", "--output", "json", "Todo Item"]),
    );

    assert_eq!(
        pads(result.stdout()),
        vec![("Todo Item".to_string(), "Todo Item".to_string())]
    );
}

#[test]
#[serial]
fn edit_takes_content_from_piped_stdin() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "Orig", "");
    drop(state);

    let (app, cmd) = fx.app(&["list"]);
    let result = TestHarness::new()
        .no_color()
        .piped_stdin("NewTitle\n\nNewBody")
        .run(&app, cmd, fx.argv(&["edit", "--output", "json", "1"]));

    assert_eq!(
        pads(result.stdout()),
        vec![("NewTitle".to_string(), "NewTitle\n\nNewBody".to_string())]
    );
}

#[test]
#[serial]
fn edit_with_an_empty_pipe_errors_and_leaves_the_pad_alone() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "Orig", "");
    drop(state);

    let (app, cmd) = fx.app(&["list"]);
    let result = TestHarness::new().no_color().piped_stdin("").run(
        &app,
        cmd,
        fx.argv(&["edit", "--output", "json", "1"]),
    );

    // Unlike create's warning, an empty pipe is a hard error for edit.
    result.assert_error();
    result.assert_exit_status(ExitStatus::FAILURE);
    result.assert_error_kind(RunErrorKind::Handler);
    result.assert_error_contains("Aborted: empty content");
    drop(result);

    let (app, cmd) = fx.read_app();
    let listed =
        TestHarness::new()
            .no_color()
            .run(&app, cmd, fx.argv(&["list", "--output", "json"]));
    assert_eq!(
        pads(listed.stdout())[0].0,
        "Orig",
        "the pad must be untouched"
    );
}

#[test]
#[serial]
fn todos_edit_uses_inline_words_over_stdin() {
    let fx = Fixture::new();
    fx.todos_mode();
    let state = fx.app_state();
    fx.seed_pad(&state, "T1", "");
    drop(state);

    let (app, cmd) = fx.app(&["list"]);
    let result = TestHarness::new().no_color().piped_stdin("PIPED").run(
        &app,
        cmd,
        fx.argv(&["edit", "--output", "json", "1", "Edited", "text"]),
    );

    assert_eq!(
        pads(result.stdout()),
        vec![("Edited text".to_string(), "Edited text".to_string())]
    );
}

#[test]
#[serial]
fn todos_edit_expands_literal_newlines() {
    let fx = Fixture::new();
    fx.todos_mode();
    let state = fx.app_state();
    fx.seed_pad(&state, "T1", "");
    drop(state);

    let (app, cmd) = fx.app(&["list"]);
    let result = TestHarness::new().no_color().interactive_stdin().run(
        &app,
        cmd,
        fx.argv(&["edit", "--output", "json", "1", r"Edited\nBody"]),
    );

    assert_eq!(
        pads(result.stdout()),
        vec![("Edited".to_string(), "Edited\n\nBody".to_string())]
    );
}

#[test]
#[serial]
fn open_shares_edits_input_resolution() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "Orig", "");
    drop(state);

    // `open` aliases `edit`'s handler; its explicit declarative registration
    // must supply the same named input or typed retrieval fails outright.
    let (app, cmd) = fx.app(&["list"]);
    let result = TestHarness::new()
        .no_color()
        .piped_stdin("ViaOpen\n\nBody")
        .run(&app, cmd, fx.argv(&["open", "--output", "json", "1"]));

    assert_eq!(
        pads(result.stdout()),
        vec![("ViaOpen".to_string(), "ViaOpen\n\nBody".to_string())]
    );
}

/// The arm the subprocess suite could not reach: a *terminal* stdin means "open
/// the editor", not "read an empty pipe".
///
/// `$EDITOR` is pointed at `/bin/false` so the editor arm is proven to be
/// *chosen* without a real editor ever succeeding: padz creates the pad, the
/// editor fails, and padz removes the half-created pad. What is asserted is the
/// routing — that an interactive stdin does not abort as an empty pipe would.
/// Whether a *working* editor round-trips text is a real-process concern and
/// stays in `editor_e2e.rs`.
#[test]
#[serial]
fn a_terminal_stdin_routes_create_to_the_editor() {
    let fx = Fixture::new();
    let (app, cmd) = fx.app(&["create"]);

    let result = TestHarness::new()
        .no_color()
        .interactive_stdin()
        .env("EDITOR", "/bin/false")
        .run(&app, cmd, fx.argv(&["create", "--output", "json"]));

    assert!(
        result.is_error(),
        "a failing editor must surface as an error, got: {:?}",
        result.stdout()
    );
    let err = result.error().unwrap_or_default().to_string();
    assert!(
        !err.contains("Aborted: empty content"),
        "an interactive stdin must route to the editor, not the empty-pipe abort: {err}"
    );
    drop(result);

    // The failed editor must not leave the half-created pad behind.
    let (app, cmd) = fx.read_app();
    let listed =
        TestHarness::new()
            .no_color()
            .run(&app, cmd, fx.argv(&["list", "--output", "json"]));
    assert_eq!(
        pads(listed.stdout()),
        vec![],
        "a failed editor create must roll the pad back"
    );
}

// =============================================================================
// Templates and text output — the wording a user reads
// =============================================================================

#[test]
#[serial]
fn an_empty_store_renders_the_create_hint() {
    let fx = Fixture::new();
    let (app, cmd) = fx.read_app();

    let result = TestHarness::new()
        .no_color()
        .text_output()
        .run(&app, cmd, fx.argv(&["list"]));

    result.assert_success();
    result.assert_exit_status(ExitStatus::SUCCESS);
    assert_eq!(result.success_kind(), Some(SuccessKind::Command));
    result.assert_stdout_contains("No pads yet, create one with `padz create`");
}

#[test]
#[serial]
fn a_filtered_listing_that_matches_nothing_says_so() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "a pad", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().text_output().run(
        &app,
        cmd,
        fx.argv(&["list", "--search", "nothing-matches-this"]),
    );

    // "No matching pads" and "No pads yet" are different facts; a filtered miss
    // must not tell the user their store is empty.
    result.assert_stdout_contains("No matching pads.");
    assert!(!result.stdout().contains("No pads yet"));
}

#[test]
#[serial]
fn a_modification_renders_its_action_verb() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new()
        .no_color()
        .terminal_width(80)
        .text_output()
        .run(&app, cmd, fx.argv(&["pin", "1"]));

    result.assert_success();
    result.assert_stdout_contains("Pinned");
    result.assert_stdout_contains("target");
}

#[test]
#[serial]
fn a_semantic_pin_notice_renders_compatible_human_wording() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    state
        .with_api(|api| api.pin_pads(state.scope, &["1"]))
        .unwrap();
    drop(state);

    let (app, cmd) = fx.read_app();
    let result =
        TestHarness::new()
            .no_color()
            .text_output()
            .run(&app, cmd, fx.argv(&["pin", "p1"]));

    result.assert_success();
    result.assert_stdout_contains("Pad p1 is already pinned");
}

#[test]
#[serial]
fn semantic_pin_notice_is_machine_readable() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "target", "");
    state
        .with_api(|api| api.pin_pads(state.scope, &["1"]))
        .unwrap();
    drop(state);

    let (app, cmd) = fx.read_app();
    let result =
        TestHarness::new()
            .no_color()
            .run(&app, cmd, fx.argv(&["pin", "p1", "--output", "json"]));
    let value: serde_json::Value = serde_json::from_str(result.stdout()).unwrap();

    assert_eq!(value["notices"][0]["kind"], "already_pinned");
    assert_eq!(value["notices"][0]["path"][0]["type"], "Pinned");
    assert_eq!(value["notices"][0]["path"][0]["value"], 1);
    assert!(value["messages"].as_array().is_some_and(Vec::is_empty));
}

#[test]
#[serial]
fn indented_view_is_shaped_by_the_template_not_the_result() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "parent", "parent body");
    fx.seed_child(&state, "1", "child", "child body\nsecond line");
    drop(state);

    let (app, cmd) = fx.read_app();
    let human = TestHarness::new().no_color().text_output().run(
        &app,
        cmd,
        fx.argv(&["view", "1", "--indented"]),
    );
    human.assert_stdout_contains("    child");
    human.assert_stdout_contains("    child body\n    second line");
    drop(human);

    let (app, cmd) = fx.read_app();
    let structured = TestHarness::new().no_color().run(
        &app,
        cmd,
        fx.argv(&["view", "1", "--indented", "--output", "json"]),
    );
    let value: serde_json::Value = serde_json::from_str(structured.stdout()).unwrap();
    assert_eq!(value["nesting"], "indented");
    assert_eq!(value["pads"][1]["depth"], 1);
    assert_eq!(value["pads"][1]["title"], "child");
    assert_eq!(value["pads"][1]["content"], "child body\nsecond line");
}

#[test]
#[serial]
fn text_output_carries_no_ansi_escapes() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "a pad", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new()
        .no_color()
        .terminal_width(80)
        .text_output()
        .run(&app, cmd, fx.argv(&["list"]));

    assert!(
        !result.stdout().contains('\u{1b}'),
        "text mode is the pipe-safe mode; it must never emit escapes: {:?}",
        result.stdout()
    );
}

// =============================================================================
// Presentation policy that lives in the templates
//
// Wording, pluralization, glyphs, section labels and index formatting are decided
// in `templates/`, not in Rust — so `render`'s unit tests cannot reach them and
// these are the tests that hold them. They drive the real template through the
// real app, which is the only place that policy exists as behaviour.
// =============================================================================

/// The count and the noun have to agree, and the noun is chosen in the template.
///
/// The two cases use separate fixtures on purpose: a pinned pad is
/// delete-protected, so reusing one store would make the plural case fail for a
/// reason that has nothing to do with wording.
#[test]
#[serial]
fn a_modification_pluralizes_its_noun_to_match_the_count() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "only one", "");
    drop(state);
    let (app, cmd) = fx.read_app();
    TestHarness::new()
        .no_color()
        .terminal_width(80)
        .text_output()
        .run(&app, cmd, fx.argv(&["pin", "1"]))
        .assert_stdout_contains("Pinned 1 pad...");

    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "first", "");
    fx.seed_pad(&state, "second", "");
    drop(state);
    let (app, cmd) = fx.read_app();
    TestHarness::new()
        .no_color()
        .terminal_width(80)
        .text_output()
        .run(&app, cmd, fx.argv(&["delete", "1", "2"]))
        .assert_stdout_contains("Deleted 2 pads...");
}

/// `--all` labels each lifecycle block. The labels are template strings, and the
/// break is driven by the section every row carries.
#[test]
#[serial]
fn the_all_listing_labels_its_lifecycle_sections() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "live pad", "");
    fx.seed_pad(&state, "gone pad", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    TestHarness::new()
        .no_color()
        .terminal_width(80)
        .text_output()
        .run(&app, cmd, fx.argv(&["delete", "1"]))
        .assert_success();

    let (app, cmd) = fx.read_app();
    let out = TestHarness::new()
        .no_color()
        .terminal_width(80)
        .text_output()
        .run(&app, cmd, fx.argv(&["list", "--all"]));

    out.assert_success();
    out.assert_stdout_contains("Deleted Pads");
    assert!(
        out.stdout().find("live pad") < out.stdout().find("Deleted Pads"),
        "the deleted block and its label come after the live pads: {}",
        out.stdout()
    );
}

/// A pinned root is listed twice — once in the pinned block, once in its own —
/// and each index format is built in the template from a typed DisplayIndex.
#[test]
#[serial]
fn a_pinned_pad_is_indexed_p1_in_the_pinned_block_and_numbered_below() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "pinned pad", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    TestHarness::new()
        .no_color()
        .terminal_width(80)
        .text_output()
        .run(&app, cmd, fx.argv(&["pin", "1"]))
        .assert_success();

    let (app, cmd) = fx.read_app();
    let out = TestHarness::new()
        .no_color()
        .terminal_width(80)
        .text_output()
        .run(&app, cmd, fx.argv(&["list"]));

    out.assert_success();
    assert!(
        out.stdout().contains("p1."),
        "pinned block: {}",
        out.stdout()
    );
    assert!(
        out.stdout().contains(" 1."),
        "regular block: {}",
        out.stdout()
    );
}

/// Status glyphs are a template lookup keyed by the serialized TodoStatus, and
/// they appear only when the listing asked for them.
#[test]
#[serial]
fn status_glyphs_appear_only_when_the_listing_asks_for_them() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "a task", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let off = TestHarness::new().no_color().terminal_width(80).run(
        &app,
        cmd,
        fx.argv(&["list", "--output", "term-debug"]),
    );
    assert!(
        !off.stdout().contains("[status-icon]"),
        "notes mode draws no status column: {}",
        off.stdout()
    );

    let (app, cmd) = fx.read_app();
    let on = TestHarness::new().no_color().terminal_width(80).run(
        &app,
        cmd,
        fx.argv(&["list", "--show-status", "--output", "term-debug"]),
    );
    assert!(
        on.stdout().contains("[status-icon]"),
        "--show-status draws the status column: {}",
        on.stdout()
    );
}

// =============================================================================
// Styles — asserted semantically via term-debug, never by scraping ANSI
// =============================================================================

#[test]
#[serial]
fn mutation_outcomes_keep_the_established_info_and_success_styles() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "before", "");
    drop(state);

    let (app, cmd) = fx.app(&["list"]);
    let result = TestHarness::new().piped_stdin("after").run(
        &app,
        cmd,
        fx.argv(&["edit", "1", "--output", "term-debug"]),
    );

    result.assert_success();
    result.assert_stdout_contains("[info]Updated 1 pad...[/info]");
    result.assert_stdout_contains("[success]Updated (1): after[/success]");
}

#[test]
#[serial]
fn term_debug_places_the_semantic_styles_on_a_pad_line() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "styled pad", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().terminal_width(80).run(
        &app,
        cmd,
        fx.argv(&["list", "--output", "term-debug"]),
    );

    let out = result.stdout();
    // term-debug keeps style tags as `[name]...[/name]`, so placement can be
    // asserted by name. Checking the *name* is what survives a palette change —
    // an ANSI-code assertion would break the moment a color is retuned.
    assert!(
        out.contains("[list-index]"),
        "index must carry list-index: {out}"
    );
    assert!(
        out.contains("[list-title]styled pad"),
        "the title must carry list-title, and carry the title: {out}"
    );
    assert!(
        out.contains("[time]"),
        "the timestamp must carry time: {out}"
    );
}

#[test]
#[serial]
fn term_debug_marks_a_deleted_pad_with_its_own_styles() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "doomed", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().terminal_width(80).run(
        &app,
        cmd,
        fx.argv(&["list", "--deleted", "--output", "term-debug"]),
    );
    drop(result);

    let (app, cmd) = fx.read_app();
    let deleted = TestHarness::new()
        .no_color()
        .run(&app, cmd, fx.argv(&["delete", "1"]));
    deleted.assert_success();
    drop(deleted);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().terminal_width(80).run(
        &app,
        cmd,
        fx.argv(&["list", "--deleted", "--output", "term-debug"]),
    );

    let out = result.stdout();
    assert!(
        out.contains("[deleted-title]") && out.contains("[deleted-index]"),
        "a deleted listing must use the deleted styles, not the active ones: {out}"
    );
}

// =============================================================================
// UUID selections — selector ordering, human rendering, and structured data
// =============================================================================

#[test]
#[serial]
fn uuid_renders_single_multiple_and_range_selections_in_order() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "first", "");
    fx.seed_pad(&state, "second", "");
    fx.seed_pad(&state, "third", "");
    drop(state);

    let mut ids = Vec::new();
    for selector in ["1", "2", "3"] {
        let (app, cmd) = fx.read_app();
        let result = TestHarness::new().no_color().text_output().run(
            &app,
            cmd,
            fx.argv(&["uuid", selector]),
        );
        result.assert_success();
        let lines: Vec<&str> = result.stdout().lines().collect();
        assert_eq!(lines.len(), 1, "{selector}: one UUID per line");
        uuid::Uuid::parse_str(lines[0])
            .unwrap_or_else(|e| panic!("{selector}: invalid UUID {:?}: {e}", lines[0]));
        ids.push(lines[0].to_string());
    }

    let (app, cmd) = fx.read_app();
    let multiple =
        TestHarness::new()
            .no_color()
            .text_output()
            .run(&app, cmd, fx.argv(&["uuid", "3", "1"]));
    multiple.assert_success();
    assert_eq!(
        multiple.stdout().lines().collect::<Vec<_>>(),
        vec![ids[2].as_str(), ids[0].as_str()],
        "multiple selectors must retain selector order"
    );
    drop(multiple);

    let (app, cmd) = fx.read_app();
    let range =
        TestHarness::new()
            .no_color()
            .text_output()
            .run(&app, cmd, fx.argv(&["uuid", "1-3"]));
    range.assert_success();
    assert_eq!(
        range.stdout().lines().collect::<Vec<_>>(),
        ids.iter().map(String::as_str).collect::<Vec<_>>(),
        "ranges must expand in canonical display order"
    );
}

// =============================================================================
// Structured output — every mode parses with a real parser for that format
// =============================================================================

#[test]
#[serial]
fn uuid_preserves_its_array_contract_in_every_structured_mode() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "structured UUID", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let text = TestHarness::new()
        .no_color()
        .text_output()
        .run(&app, cmd, fx.argv(&["uuid", "1"]));
    text.assert_success();
    let expected = text.stdout().trim().to_string();
    uuid::Uuid::parse_str(&expected).expect("human output should contain a full UUID");
    drop(text);

    for mode in ["json", "yaml", "xml", "csv"] {
        let (app, cmd) = fx.read_app();
        let result =
            TestHarness::new()
                .no_color()
                .run(&app, cmd, fx.argv(&["uuid", "1", "--output", mode]));
        result.assert_success();

        let actual = match mode {
            "json" => {
                let value: serde_json::Value = serde_json::from_str(result.stdout())
                    .unwrap_or_else(|e| panic!("not JSON: {e}\n{}", result.stdout()));
                value["uuids"][0].as_str().expect("uuids[0]").to_string()
            }
            "yaml" => {
                let value: serde_json::Value = serde_yaml::from_str(result.stdout())
                    .unwrap_or_else(|e| panic!("not YAML: {e}\n{}", result.stdout()));
                value["uuids"][0].as_str().expect("uuids[0]").to_string()
            }
            "xml" => {
                let mut reader = quick_xml::Reader::from_str(result.stdout());
                reader.config_mut().trim_text(true);
                let mut in_uuid = false;
                let mut uuid = None;
                loop {
                    match reader.read_event() {
                        Ok(quick_xml::events::Event::Start(e)) if e.name().as_ref() == b"uuids" => {
                            in_uuid = true;
                        }
                        Ok(quick_xml::events::Event::Text(e)) if in_uuid => {
                            uuid = Some(e.unescape().expect("UUID XML text").into_owned());
                        }
                        Ok(quick_xml::events::Event::End(e)) if e.name().as_ref() == b"uuids" => {
                            in_uuid = false;
                        }
                        Ok(quick_xml::events::Event::Eof) => break,
                        Ok(_) => {}
                        Err(e) => panic!("not XML: {e}\n{}", result.stdout()),
                    }
                }
                uuid.expect("XML uuids element")
            }
            "csv" => {
                let mut reader = csv::Reader::from_reader(result.stdout().as_bytes());
                assert_eq!(
                    reader.headers().expect("CSV header").get(0),
                    Some("uuids.0")
                );
                reader
                    .records()
                    .next()
                    .expect("CSV row")
                    .expect("valid CSV row")[0]
                    .to_string()
            }
            other => unreachable!("unhandled mode {other}"),
        };

        assert_eq!(actual, expected, "{mode} UUID array changed");
    }
}

#[test]
#[serial]
fn json_output_parses_and_carries_the_pads() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "json pad", "json body");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result =
        TestHarness::new()
            .no_color()
            .run(&app, cmd, fx.argv(&["list", "--output", "json"]));

    assert_eq!(
        pads(result.stdout()),
        vec![("json pad".to_string(), "json pad\n\njson body".to_string())]
    );
}

#[test]
#[serial]
fn yaml_output_parses_and_agrees_with_json() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "shared pad", "shared body");
    drop(state);

    let (app, cmd) = fx.read_app();
    let json = TestHarness::new()
        .no_color()
        .run(&app, cmd, fx.argv(&["list", "--output", "json"]));
    let json_titles: Vec<String> = pads(json.stdout()).into_iter().map(|(t, _)| t).collect();
    drop(json);

    let (app, cmd) = fx.read_app();
    let result =
        TestHarness::new()
            .no_color()
            .run(&app, cmd, fx.argv(&["list", "--output", "yaml"]));

    let v: serde_json::Value = serde_yaml::from_str(result.stdout())
        .unwrap_or_else(|e| panic!("not YAML: {e}\n{}", result.stdout()));
    let yaml_titles: Vec<String> = v["pads"]
        .as_array()
        .expect("pads sequence")
        .iter()
        .map(|p| p["pad"]["metadata"]["title"].as_str().unwrap().to_string())
        .collect();

    // The formats are two encodings of one result; disagreement means a mode is
    // deriving its own data rather than serializing the handler's.
    assert_eq!(yaml_titles, json_titles);
}

#[test]
#[serial]
fn xml_output_parses_and_carries_the_title() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "xml pad", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result =
        TestHarness::new()
            .no_color()
            .run(&app, cmd, fx.argv(&["list", "--output", "xml"]));

    let mut reader = quick_xml::Reader::from_str(result.stdout());
    let mut depth = 0usize;
    let mut in_title = false;
    let mut titles: Vec<String> = Vec::new();
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(e)) => {
                depth += 1;
                in_title = e.name().as_ref() == b"title";
            }
            // Read the title through the parser's own unescaping: only the parser
            // can say whether `<title>` was markup or an escaped literal, which is
            // the distinction this test exists to make.
            Ok(quick_xml::events::Event::Text(e)) if in_title => titles.push(
                e.unescape()
                    .unwrap_or_else(|err| panic!("undecodable title text: {err}"))
                    .into_owned(),
            ),
            Ok(quick_xml::events::Event::End(_)) => {
                depth -= 1;
                in_title = false;
            }
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("not well-formed XML: {e}\n{}", result.stdout()),
        }
    }
    assert_eq!(depth, 0, "XML tags must balance");
    assert!(
        titles.iter().any(|t| t == "xml pad"),
        "the title must be a real <title> element, not text: {}",
        result.stdout()
    );
}

#[test]
#[serial]
fn csv_output_parses_with_a_matching_header_and_row() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "csv pad", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result =
        TestHarness::new()
            .no_color()
            .run(&app, cmd, fx.argv(&["list", "--output", "csv"]));

    let mut rdr = csv::Reader::from_reader(result.stdout().as_bytes());
    let headers = rdr.headers().expect("CSV header").clone();
    let rows: Vec<_> = rdr.records().collect::<Result<_, _>>().expect("CSV rows");

    assert_eq!(rows.len(), 1, "one flattened row per result");
    assert_eq!(
        rows[0].len(),
        headers.len(),
        "every row must match the header's field count"
    );
    assert!(
        headers.iter().any(|h| h.ends_with("metadata.title")),
        "the flattened header must name the title column: {headers:?}"
    );
}

#[test]
#[serial]
fn structured_output_is_invariant_across_terminal_width() {
    // Migrated from `terminal_width_e2e.rs`, which set `COLUMNS` on a spawned
    // process and asserted the JSON still parsed. That child had no pty, so its
    // width came from the detector's fallback either way and the env var proved
    // little. Here the width detector is injected, so a template width actually
    // leaking into structured output would fail this.
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "a pad with a reasonably long title", "body");
    fx.seed_pad(&state, "another title", "body");
    drop(state);

    for command in [
        vec!["list", "--output", "json"],
        vec!["search", "title", "--output", "json"],
    ] {
        let mut seen: Option<Vec<(String, String)>> = None;
        for width in [20usize, 80, 200] {
            let (app, cmd) = fx.read_app();
            let result = TestHarness::new().no_color().terminal_width(width).run(
                &app,
                cmd,
                fx.argv(&command),
            );

            let got = pads(result.stdout());
            assert!(
                !got.is_empty(),
                "{command:?} at {width} cols returned nothing"
            );
            match &seen {
                None => seen = Some(got),
                Some(first) => assert_eq!(
                    &got, first,
                    "{command:?}: structured output must not vary with terminal width (at {width} cols)"
                ),
            }
        }
    }
}

#[test]
#[serial]
fn structured_output_excludes_template_only_view_fields() {
    const TEMPLATE_ONLY: [&str; 5] = ["line_width", "title_width", "indent", "time_ago", "cols"];
    // The title deliberately spells the forbidden fields. Only a *key* named
    // `indent` is a leak; a pad that merely talks about indentation is data the
    // result must carry verbatim. Seeding both into one document is what keeps
    // this test honest — it fails against a `contains` check on raw stdout.
    const TITLE: &str = "notes on indent, time_ago and cols";

    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, TITLE, "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().terminal_width(80).run(
        &app,
        cmd,
        fx.argv(&["list", "--output", "json"]),
    );

    let out = result.stdout();
    let parsed: serde_json::Value =
        serde_json::from_str(out).unwrap_or_else(|e| panic!("not JSON: {e}\n{out}"));
    let shape = shape_of(&parsed);

    // These are derived at render time by the view builders. Their presence here
    // would mean a context provider ran for a structured mode — the exact leak
    // the result/view split exists to prevent.
    for leaked in TEMPLATE_ONLY {
        assert!(
            !shape.keys.contains(leaked),
            "structured output leaked the template-only field {leaked:?} as a key: {out}"
        );
    }

    // The other half of the contract: the parse must not have bought its pass by
    // dropping the pad. Without this, deleting the title would satisfy the loop.
    assert!(
        shape.strings.iter().any(|s| s == TITLE),
        "the title must survive verbatim as a value: {out}"
    );
}

#[test]
#[serial]
fn an_empty_result_stays_valid_in_every_structured_mode() {
    let fx = Fixture::new();

    for mode in ["json", "yaml"] {
        let (app, cmd) = fx.read_app();
        let result =
            TestHarness::new()
                .no_color()
                .run(&app, cmd, fx.argv(&["list", "--output", mode]));

        // An empty listing is a normal result, not an error or a human hint.
        assert!(
            result.is_handled(),
            "{mode}: an empty listing must still render"
        );

        // Parse with the real parser for the format: human output is not valid
        // YAML, so parsing is what actually detects a fallback to the template.
        let parsed: serde_json::Value = match mode {
            "json" => serde_json::from_str(result.stdout())
                .unwrap_or_else(|e| panic!("{mode}: not valid JSON: {e}\n{}", result.stdout())),
            "yaml" => serde_yaml::from_str(result.stdout())
                .unwrap_or_else(|e| panic!("{mode}: not valid YAML: {e}\n{}", result.stdout())),
            other => unreachable!("unhandled mode {other}"),
        };

        assert!(
            parsed["pads"].as_array().is_some_and(|a| a.is_empty()),
            "{mode}: an empty listing must serialize an empty pads array: {parsed:?}"
        );
        // Valid JSON is not enough: a structured mode could still serialize the
        // human hint as a field. Ask the parsed document, not the raw text —
        // scraping stdout here would answer a different question than the one
        // this assertion is asking.
        let shape = shape_of(&parsed);
        let leaked: Vec<&String> = shape
            .strings
            .iter()
            .filter(|s| s.contains("No pads yet"))
            .collect();
        assert!(
            leaked.is_empty(),
            "{mode}: the human empty-state hint must not leak into structured output: {leaked:?}"
        );
    }
}

// =============================================================================
// Output file
// =============================================================================

#[test]
#[serial]
fn export_artifact_uses_the_explicit_destination_and_reports_its_receipt() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "exported", "body");
    drop(state);

    let target = fx.root().join("chosen.tar.gz");
    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().run(
        &app,
        cmd,
        fx.argv(&["export", "--output-file-path", target.to_str().unwrap()]),
    );

    result.assert_success();
    assert_eq!(&result.artifact_bytes().unwrap()[..2], &[0x1f, 0x8b]);
    result.assert_artifact_written_to(&target);
    result.assert_artifact_report_contains(&format!("Exported to {}", target.display()));
    assert_eq!(
        std::fs::read(&target).unwrap(),
        result.artifact_bytes().unwrap()
    );
}

#[test]
#[serial]
fn export_artifact_report_is_machine_readable_and_keeps_metadata_warnings() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "plain text", "body");
    drop(state);

    let target = fx.root().join("metadata.tar.gz");
    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().run(
        &app,
        cmd,
        fx.argv(&[
            "export",
            "--with-metadata",
            "--output",
            "json",
            "--output-file-path",
            target.to_str().unwrap(),
        ]),
    );

    result.assert_success();
    let report: serde_json::Value =
        serde_json::from_str(result.artifact_report().unwrap()).unwrap();
    assert_eq!(report["report"]["status"], "exported");
    assert_eq!(report["report"]["format"], "metadata_archive");
    assert_eq!(report["report"]["exported"], 1);
    assert_eq!(
        report["report"]["warnings"][0]["kind"],
        "metadata_unavailable"
    );
    assert_eq!(report["report"]["warnings"][0]["titles"][0], "plain text");
    assert_eq!(
        report["receipt"]["destination"],
        target.display().to_string()
    );
}

#[test]
#[serial]
fn export_artifact_uses_its_suggested_single_file_destination() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "one", "body");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().cwd(fx.root()).run(
        &app,
        cmd,
        fx.argv(&["export", "--single-file", "My / Export.md"]),
    );

    result.assert_success();
    result.assert_artifact_suggested_destination("My _ Export.md");
    result.assert_artifact_written_to("My _ Export.md");
    result.assert_artifact_report_contains("Exported 1 pads to My _ Export.md");
    let written = std::fs::read_to_string(fx.root().join("My _ Export.md")).unwrap();
    assert!(written.contains("## one"));
}

#[test]
#[serial]
fn empty_export_remains_non_artifact_output() {
    let fx = Fixture::new();
    let (app, cmd) = fx.read_app();
    let result = TestHarness::new()
        .no_color()
        .run(&app, cmd, fx.argv(&["export"]));

    result.assert_success();
    assert!(result.artifact().is_none());
    result.assert_stdout_contains("No pads to export.");
}

#[test]
#[serial]
fn artifact_write_failure_is_typed_and_emits_no_success_report() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "unwritten", "");
    drop(state);

    let target = fx.root();
    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().run(
        &app,
        cmd,
        fx.argv(&["export", "--output-file-path", target.to_str().unwrap()]),
    );

    result.assert_error();
    result.assert_exit_status(ExitStatus::FAILURE);
    result.assert_error_kind(RunErrorKind::FinalWrite(OutputKind::Artifact));
    assert!(
        result.artifact().is_none(),
        "failed writes have no receipt/report"
    );
    assert!(!result.error().unwrap_or_default().contains("Exported to"));
}

#[test]
#[serial]
fn output_file_path_writes_the_result_to_the_file_and_stays_silent() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "written pad", "");
    drop(state);

    let target = fx.root().join("out.json");
    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().run(
        &app,
        cmd,
        fx.argv(&[
            "list",
            "--output",
            "json",
            "--output-file-path",
            target.to_str().unwrap(),
        ]),
    );

    result.assert_success();
    result.assert_exit_status(ExitStatus::SUCCESS);
    assert_eq!(result.success_kind(), Some(SuccessKind::Command));
    assert_eq!(
        result.stdout(),
        "",
        "output redirected to a file must not also print to stdout"
    );

    let written = std::fs::read_to_string(&target).expect("the output file must exist");
    assert_eq!(
        pads(&written),
        vec![("written pad".to_string(), "written pad".to_string())],
        "the file must hold the same structured result stdout would have carried"
    );
}

#[test]
#[serial]
fn output_file_path_writes_text_without_ansi_escapes() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "written pad", "");
    drop(state);

    let target = fx.root().join("out.txt");
    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().with_color().terminal_width(80).run(
        &app,
        cmd,
        fx.argv(&[
            "list",
            "--output",
            "term",
            "--output-file-path",
            target.to_str().unwrap(),
        ]),
    );
    result.assert_success();

    let written = std::fs::read_to_string(&target).expect("the output file must exist");
    assert!(
        !written.contains('\u{1b}'),
        "a file is not a terminal; the written output must be escape-free: {written:?}"
    );
    assert!(written.contains("written pad"));
}

#[test]
#[serial]
fn output_file_path_reports_a_typed_final_write_failure() {
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "unwritten pad", "");
    drop(state);

    // A directory is a deterministic invalid file target on every supported
    // platform, without depending on permission bits or the test user's uid.
    let target = fx.root();
    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().run(
        &app,
        cmd,
        fx.argv(&[
            "list",
            "--output",
            "json",
            "--output-file-path",
            target.to_str().unwrap(),
        ]),
    );

    result.assert_error();
    result.assert_exit_status(ExitStatus::FAILURE);
    result.assert_error_kind(RunErrorKind::FinalWrite(OutputKind::Text));
    result.assert_error_contains("Error writing output");
}

// =============================================================================
// Search-hit layout
// =============================================================================

/// The display column `needle` starts at, counted the way a terminal counts.
///
/// Byte and `char` offsets both lie here: the status glyph is two `char`s
/// (symbol + variation selector) and one column. This measures with the crate
/// the renderer measures with, so a column here means a column there.
fn column_of(line: &str, needle: &str) -> usize {
    let byte = line
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not found in {line:?}"));
    UnicodeWidthStr::width(&line[..byte])
}

/// A search hit hangs its `04L ` badge in the gutter and lands its *text* on the
/// pad's title column, so a hit reads as a continuation of the title it matched.
///
/// That column is not a constant: it moves with `--show-status`, because the
/// status column only costs width when it is asked for. The badge offset must
/// therefore be derived from the same column maths the pad line uses.
/// Regression guard — a hard-coded gutter lines up in whichever configuration it
/// was tuned against and silently drifts by the status width in the other, which
/// is exactly what it used to do.
///
/// Only depth 0 is asserted, because only depth 0 is reachable: `apply_search`
/// filters the root pads and never recurses into their children, so a pad that
/// carries hits is always a root. The template still derives the badge from
/// `pad.depth`, which costs nothing and stays correct if search ever descends.
#[test]
#[serial]
fn search_hit_text_lands_on_the_title_column_in_every_configuration() {
    for show_status in [false, true] {
        let fx = Fixture::new();
        let state = fx.app_state();
        fx.seed_pad(&state, "hit pad", "alpha\nneedle here\n");
        drop(state);

        let mut argv = vec!["list", "--search", "needle"];
        if show_status {
            argv.push("--show-status");
        }

        let (app, cmd) = fx.read_app();
        let result = TestHarness::new()
            .no_color()
            .terminal_width(80)
            .text_output()
            .run(&app, cmd, fx.argv(&argv));

        result.assert_success();
        let stdout = result.stdout();
        let case = format!("show_status={show_status}");

        let title_line = stdout
            .lines()
            .find(|l| l.contains("hit pad"))
            .unwrap_or_else(|| panic!("no pad line ({case}):\n{stdout}"));
        let hit_line = stdout
            .lines()
            .find(|l| l.contains("needle here"))
            .unwrap_or_else(|| panic!("no hit line ({case}):\n{stdout}"));

        assert_eq!(
            column_of(hit_line, "needle here"),
            column_of(title_line, "hit pad"),
            "hit text must start on the title column ({case}):\n{stdout}"
        );
    }
}

// =============================================================================
// The guard: this file's own serial rule, enforced mechanically
// =============================================================================

/// Every `#[test]` in this file must also be `#[serial]`.
///
/// The harness mutates process-global seams, so a non-serial harness test is a
/// latent flake that passes until the day the scheduler interleaves it with
/// another. Review is not a reliable check for "did someone forget an
/// attribute", so this reads the file and checks it.
///
/// It exempts itself: it touches no harness.
#[test]
fn every_harness_test_is_serial() {
    let source = include_str!("harness.rs");
    let lines: Vec<&str> = source.lines().collect();

    let mut offenders = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() != "#[test]" {
            continue;
        }
        // The attributes between `#[test]` and the `fn` line.
        let attrs: Vec<&str> = lines[i + 1..]
            .iter()
            .take_while(|l| l.trim_start().starts_with('#'))
            .copied()
            .collect();
        let fn_line = lines[i + 1..]
            .iter()
            .find(|l| l.trim_start().starts_with("fn "))
            .copied()
            .unwrap_or("<unknown>");

        if fn_line.contains("fn every_harness_test_is_serial") {
            continue;
        }
        if !attrs.iter().any(|a| a.trim() == "#[serial]") {
            offenders.push(fn_line.trim().to_string());
        }
    }

    assert!(
        offenders.is_empty(),
        "these harness tests are missing #[serial]: {offenders:#?}"
    );
}
