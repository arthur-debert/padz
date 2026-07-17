//! `TestHarness` integration tests — layer 3 of the pyramid (see `src/lib.rs`).
//!
//! # The seam this file protects
//!
//! Everything between argv and rendered output, in process: clap parsing, the
//! pre-dispatch input chains, dispatch to the right handler, the view builders,
//! the templates, the stylesheet, and the output-mode matrix. It is the only
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

mod support;

use standout_test::{serial, TestHarness};
use support::Fixture;

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

    // Clap rejects it before dispatch; the run must not quietly succeed.
    assert!(
        !result.is_handled() || result.is_error(),
        "an unknown command must not render as a normal result: {:?}",
        result.stdout()
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
    assert!(
        result.is_error(),
        "an empty pipe must fail the edit, got: {:?}",
        result.stdout()
    );
    assert!(
        result
            .error()
            .unwrap_or_default()
            .contains("Aborted: empty content"),
        "got: {:?}",
        result.error()
    );
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

    // `open` aliases `edit`'s handler; without its own chain registration the
    // handler's input lookup would fail outright.
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
// Styles — asserted semantically via term-debug, never by scraping ANSI
// =============================================================================

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
// Structured output — every mode parses with a real parser for that format
// =============================================================================

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
    loop {
        match reader.read_event() {
            Ok(quick_xml::events::Event::Start(_)) => depth += 1,
            Ok(quick_xml::events::Event::End(_)) => depth -= 1,
            Ok(quick_xml::events::Event::Eof) => break,
            Ok(_) => {}
            Err(e) => panic!("not well-formed XML: {e}\n{}", result.stdout()),
        }
    }
    assert_eq!(depth, 0, "XML tags must balance");
    assert!(
        result.stdout().contains("<title>xml pad</title>"),
        "the title must be a real element, not text: {}",
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
    let fx = Fixture::new();
    let state = fx.app_state();
    fx.seed_pad(&state, "a pad", "");
    drop(state);

    let (app, cmd) = fx.read_app();
    let result = TestHarness::new().no_color().terminal_width(80).run(
        &app,
        cmd,
        fx.argv(&["list", "--output", "json"]),
    );

    let out = result.stdout();
    // These are derived at render time by the view builders. Their presence here
    // would mean a context provider ran for a structured mode — the exact leak
    // the result/view split exists to prevent.
    for leaked in ["line_width", "title_width", "indent", "time_ago", "cols"] {
        assert!(
            !out.contains(leaked),
            "structured output leaked the template-only field {leaked:?}: {out}"
        );
    }
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
        assert!(
            !result.stdout().contains("No pads yet"),
            "{mode}: the human empty-state hint must not leak into structured output: {}",
            result.stdout()
        );
    }
}

// =============================================================================
// Output file
// =============================================================================

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
