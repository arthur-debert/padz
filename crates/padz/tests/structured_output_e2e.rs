//! # Structured output contract (JSON / YAML / XML / CSV)
//!
//! These tests pin the machine-readable contract padz offers agents and scripts:
//! `--output json|yaml|xml|csv` emits *data*, never the human renderer's text.
//!
//! ## Why these tests parse instead of matching strings
//!
//! The bug this suite guards against was invisible to `contains` assertions. padz
//! extracted the output mode with a hand-written match that knew only `json`, while
//! standout's `--output` flag already accepted `yaml`, `xml`, and `csv`. Those three
//! parsed as valid clap values, fell through to `OutputMode::Auto`, and rendered the
//! *human template* — ANSI, glyphs, width truncation and all — to a caller who had
//! asked for machine-readable data. Exit code 0. The word `title` appears in that
//! human text too, so a substring assertion would have called it a pass.
//!
//! So every structured assertion here parses with a real parser for that format
//! (`serde_json`, `serde_yaml`, `quick_xml`, `csv`) and asserts on the parsed value.
//! Human output is not valid YAML/XML/CSV, so parsing is what actually detects the
//! fallback.
//!
//! ## What is asserted
//!
//! - Every command has an explicit outcome in all four formats (no accidental fallback).
//! - `path` and `uuid` — which used to print directly — produce parseable data.
//! - Structured output carries no ANSI, no style tags, no width-derived truncation,
//!   and none of the template-only view fields the render layer derives.
//! - Empty, singleton, multiple, and nested results stay valid in each format.
//! - JSON, YAML, and XML agree on semantic fields.
//! - CSV's lossy flattening is pinned deliberately (see `csv_flattening_contract`).
//! - Structured bytes are invariant across terminal width and color settings.

#![allow(deprecated)]

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

/// The four modes that must serialize data rather than render the human template.
const STRUCTURED_MODES: [&str; 4] = ["json", "yaml", "xml", "csv"];

fn padz_cmd() -> Command {
    Command::new(cargo_bin("padz"))
}

/// A project with an isolated store, so tests never touch the developer's real pads.
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

    /// Runs padz, asserting success, and returns stdout.
    fn run(&self, args: &[&str]) -> String {
        let out = self.try_run(args);
        assert!(
            out.status.success(),
            "padz {:?} failed: {}",
            args,
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("padz emitted non-UTF8 stdout")
    }

    fn try_run(&self, args: &[&str]) -> std::process::Output {
        padz_cmd()
            .env("PADZ_GLOBAL_DATA", self.global.as_os_str())
            .current_dir(&self.project)
            .args(args)
            .output()
            .unwrap()
    }

    /// Runs padz with extra env (used for the width/color invariance test).
    fn run_with_env(&self, args: &[&str], env: &[(&str, &str)]) -> String {
        let mut cmd = padz_cmd();
        cmd.env("PADZ_GLOBAL_DATA", self.global.as_os_str())
            .current_dir(&self.project);
        for (k, v) in env {
            cmd.env(k, v);
        }
        let out = cmd.args(args).output().unwrap();
        assert!(out.status.success(), "padz {:?} failed", args);
        String::from_utf8(out.stdout).unwrap()
    }

    /// Creates a pad. `--output` must precede the title: `title` is a
    /// `trailing_var_arg`, so anything after it is captured as title text.
    fn create(&self, title: &str) {
        self.run(&["create", "--no-editor", title]);
    }
}

// ---------------------------------------------------------------------------
// Parsers. Each returns a normalized serde_json::Value so formats can be compared.
// ---------------------------------------------------------------------------

fn parse_json(s: &str) -> serde_json::Value {
    serde_json::from_str(s).unwrap_or_else(|e| panic!("not valid JSON: {e}\n---\n{s}\n---"))
}

fn parse_yaml(s: &str) -> serde_json::Value {
    let v: serde_yaml::Value =
        serde_yaml::from_str(s).unwrap_or_else(|e| panic!("not valid YAML: {e}\n---\n{s}\n---"));
    serde_json::to_value(v).expect("YAML value is not JSON-representable")
}

/// Parses XML into (tag, text) pairs for leaf elements.
///
/// Deliberately shallow: standout's XML is a flat-ish element tree, and the
/// assertions here only need "which leaves carry which text", not a full DOM.
fn parse_xml_leaves(s: &str) -> Vec<(String, String)> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(s);
    reader.config_mut().trim_text(true);
    let mut leaves = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                stack.push(String::from_utf8_lossy(e.name().as_ref()).into_owned());
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Empty(e)) => {
                // Self-closing element: a present-but-null/empty field.
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                leaves.push((name, String::new()));
            }
            Ok(Event::Text(e)) => {
                if let Some(tag) = stack.last() {
                    let text = e.unescape().unwrap_or_default().into_owned();
                    if !text.is_empty() {
                        leaves.push((tag.clone(), text));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => panic!("not valid XML: {e}\n---\n{s}\n---"),
            _ => {}
        }
        buf.clear();
    }
    leaves
}

/// Parses CSV into (headers, rows). Panics if the text is not well-formed CSV.
fn parse_csv(s: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(s.as_bytes());
    let headers = rdr
        .headers()
        .unwrap_or_else(|e| panic!("not valid CSV headers: {e}\n---\n{s}\n---"))
        .iter()
        .map(|h| h.to_string())
        .collect();
    let rows = rdr
        .records()
        .map(|r| {
            r.unwrap_or_else(|e| panic!("not valid CSV row: {e}\n---\n{s}\n---"))
                .iter()
                .map(|f| f.to_string())
                .collect()
        })
        .collect();
    (headers, rows)
}

/// Parses `text` according to `mode`, panicking if it is not valid for that format.
///
/// This is the single chokepoint that catches a human-template fallback: the human
/// renderer's output is not parseable as any of these four formats.
fn assert_parses(mode: &str, text: &str) {
    match mode {
        "json" => {
            parse_json(text);
        }
        "yaml" => {
            parse_yaml(text);
        }
        "xml" => {
            parse_xml_leaves(text);
        }
        "csv" => {
            parse_csv(text);
        }
        other => panic!("unknown mode {other}"),
    }
}

// ---------------------------------------------------------------------------
// The regression test for the bug this workstream fixes.
// ---------------------------------------------------------------------------

/// `--output yaml|xml|csv` used to silently render the human template because the
/// local mode parser only knew `json`. Each must now select its standout mode.
#[test]
fn yaml_xml_csv_do_not_fall_back_to_human_rendering() {
    let fx = Fixture::new();
    fx.create("hello world");

    let human = fx.run(&["list", "--output", "term"]);

    for mode in STRUCTURED_MODES {
        let out = fx.run(&["list", "--output", mode]);
        assert_ne!(
            out.trim(),
            human.trim(),
            "--output {mode} returned the human rendering verbatim — it fell back to Auto"
        );
        assert_parses(mode, &out);
    }
}

/// The human renderer draws a `⏲` glyph and a `1.`-style index. Neither is data;
/// their presence in structured output means the template ran.
#[test]
fn structured_output_carries_no_human_render_artifacts() {
    let fx = Fixture::new();
    fx.create("hello world");

    for mode in STRUCTURED_MODES {
        let out = fx.run(&["list", "--output", mode]);
        assert!(
            !out.contains('\u{1b}'),
            "--output {mode} contains an ANSI escape:\n{out}"
        );
        for glyph in ['⏲', '⚲'] {
            assert!(
                !out.contains(glyph),
                "--output {mode} contains human glyph {glyph:?}:\n{out}"
            );
        }
        // Semantic style tags (standout markup) must never survive into data.
        for tag in ["[dim]", "[bold]", "[/]", "[muted]"] {
            assert!(
                !out.contains(tag),
                "--output {mode} contains style tag {tag}:\n{out}"
            );
        }
    }
}

/// The view builders in `cli::render` derive template-only fields (column widths,
/// glyphs, relative timestamps). Standout resolves them only on the template path,
/// so they must never appear in structured output.
#[test]
fn structured_output_excludes_template_only_view_fields() {
    let fx = Fixture::new();
    fx.create("hello world");

    let json = fx.run(&["list", "--output", "json"]);
    let v = parse_json(&json);
    let top: Vec<&str> = v.as_object().unwrap().keys().map(|k| k.as_str()).collect();

    // The result contract: data + invocation facts, nothing view-derived.
    assert_eq!(top, vec!["messages", "pads", "request"]);

    for forbidden in ["rows", "time_ago", "status_icon", "left_pin", "columns"] {
        assert!(
            !json.contains(forbidden),
            "structured JSON leaked template-only field `{forbidden}`:\n{json}"
        );
    }
}

// ---------------------------------------------------------------------------
// Every command has an explicit outcome in every structured mode.
// ---------------------------------------------------------------------------

/// Read-oriented commands must serialize in all four formats.
///
/// This is the "no command accidentally falls back" sweep. `create` is exercised
/// separately because its `trailing_var_arg` title changes the argument order.
#[test]
fn every_read_command_serializes_in_every_structured_mode() {
    let fx = Fixture::new();
    fx.create("alpha");
    fx.create("beta");

    let commands: &[&[&str]] = &[
        &["list"],
        &["list", "--peek"],
        &["list", "--uuid"],
        &["list", "--all"],
        &["search", "alpha"],
        &["peek", "1"],
        &["view", "1"],
        &["path", "1"],
        &["uuid", "1"],
        &["tag", "list"],
        &["doctor"],
    ];

    for cmd in commands {
        for mode in STRUCTURED_MODES {
            let mut args = cmd.to_vec();
            args.extend_from_slice(&["--output", mode]);
            let out = fx.run(&args);
            assert_parses(mode, &out);
            assert!(
                !out.contains('\u{1b}'),
                "{cmd:?} --output {mode} contains ANSI"
            );
        }
    }
}

/// Mutating commands return a `ModificationResult` — `action` plus the pads it
/// touched — in every structured mode.
#[test]
fn mutating_commands_serialize_in_every_structured_mode() {
    for mode in STRUCTURED_MODES {
        let fx = Fixture::new();
        fx.create("alpha");

        for (cmd, _) in [("pin", ()), ("unpin", ()), ("complete", ()), ("reopen", ())] {
            let out = fx.run(&[cmd, "1", "--output", mode]);
            assert_parses(mode, &out);
        }
    }
}

/// `create` serializes too, but `--output` must precede the title because `title`
/// is a `trailing_var_arg`. This test documents that ordering as intentional.
#[test]
fn create_serializes_when_output_flag_precedes_title() {
    for mode in STRUCTURED_MODES {
        let fx = Fixture::new();
        let out = fx.run(&["create", "--no-editor", "--output", mode, "fresh pad"]);
        assert_parses(mode, &out);
    }

    // The other ordering captures the flag as title text — intentional free-text
    // capture, and the reason agents must put --output first.
    let fx = Fixture::new();
    fx.run(&["create", "--no-editor", "swallowed", "--output", "json"]);
    let titles = titles_of(&parse_json(&fx.run(&["list", "--output", "json"])));
    assert!(
        titles.contains(&"swallowed --output json".to_string()),
        "expected trailing_var_arg to capture the flag as title text, got {titles:?}"
    );
}

// ---------------------------------------------------------------------------
// path / uuid — the two commands that used to print directly.
// ---------------------------------------------------------------------------

#[test]
fn path_produces_valid_structured_data_in_every_mode() {
    let fx = Fixture::new();
    fx.create("alpha");

    let json = parse_json(&fx.run(&["path", "1", "--output", "json"]));
    let paths = json["paths"].as_array().expect("paths must be an array");
    assert_eq!(paths.len(), 1);
    let path = paths[0].as_str().unwrap();
    assert!(path.ends_with(".txt"), "path should be a pad file: {path}");
    assert!(
        std::path::Path::new(path).exists(),
        "path must point at a real file: {path}"
    );

    // Same value, every format.
    assert_eq!(
        parse_yaml(&fx.run(&["path", "1", "--output", "yaml"])),
        json
    );

    let xml = parse_xml_leaves(&fx.run(&["path", "1", "--output", "xml"]));
    assert!(xml.iter().any(|(tag, text)| tag == "paths" && text == path));

    let (headers, rows) = parse_csv(&fx.run(&["path", "1", "--output", "csv"]));
    assert_eq!(headers, vec!["paths.0"]);
    assert_eq!(rows[0], vec![path]);
}

#[test]
fn uuid_produces_valid_structured_data_in_every_mode() {
    let fx = Fixture::new();
    fx.create("alpha");

    let json = parse_json(&fx.run(&["uuid", "1", "--output", "json"]));
    let uuids = json["uuids"].as_array().expect("uuids must be an array");
    assert_eq!(uuids.len(), 1);
    let uuid = uuids[0].as_str().unwrap();
    // Canonical 8-4-4-4-12 form, not a rendered/truncated one.
    assert_eq!(uuid.len(), 36, "expected a full uuid, got {uuid}");
    assert_eq!(uuid.matches('-').count(), 4);

    assert_eq!(
        parse_yaml(&fx.run(&["uuid", "1", "--output", "yaml"])),
        json
    );

    let xml = parse_xml_leaves(&fx.run(&["uuid", "1", "--output", "xml"]));
    assert!(xml.iter().any(|(tag, text)| tag == "uuids" && text == uuid));

    let (headers, rows) = parse_csv(&fx.run(&["uuid", "1", "--output", "csv"]));
    assert_eq!(headers, vec!["uuids.0"]);
    assert_eq!(rows[0], vec![uuid]);
}

/// A store under a directory whose name contains `[` / `]` must round-trip through
/// structured output unmangled — bracket text must not be read as style markup.
#[test]
fn path_with_brackets_is_not_treated_as_style_markup() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("pro[ject]");
    let global = temp.path().join("global");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&global).unwrap();
    fs::create_dir(project.join(".git")).unwrap();

    let run = |args: &[&str]| -> String {
        let out = padz_cmd()
            .env("PADZ_GLOBAL_DATA", global.as_os_str())
            .current_dir(&project)
            .args(args)
            .output()
            .unwrap();
        assert!(out.status.success(), "padz {args:?} failed");
        String::from_utf8(out.stdout).unwrap()
    };

    run(&["init"]);
    run(&["create", "--no-editor", "bracket pad"]);

    let json = parse_json(&run(&["path", "1", "--output", "json"]));
    let path = json["paths"][0].as_str().unwrap();
    assert!(
        path.contains("pro[ject]"),
        "brackets must survive verbatim, got {path}"
    );
    assert!(std::path::Path::new(path).exists());
}

// ---------------------------------------------------------------------------
// Cardinality: empty, singleton, multiple, nested.
// ---------------------------------------------------------------------------

/// Collects pad titles from a `PadListResult`, recursing into children.
fn titles_of(v: &serde_json::Value) -> Vec<String> {
    fn walk(pads: &serde_json::Value, out: &mut Vec<String>) {
        for p in pads.as_array().into_iter().flatten() {
            if let Some(t) = p["pad"]["metadata"]["title"].as_str() {
                out.push(t.to_string());
            }
            walk(&p["children"], out);
        }
    }
    let mut out = Vec::new();
    walk(&v["pads"], &mut out);
    out
}

#[test]
fn empty_result_stays_valid_in_every_format() {
    let fx = Fixture::new();
    fx.create("alpha");

    for mode in STRUCTURED_MODES {
        // A filter that matches nothing.
        let out = fx.run(&["list", "--search", "zzz-no-such-pad", "--output", mode]);
        assert_parses(mode, &out);
    }

    let json = parse_json(&fx.run(&["list", "--search", "zzz-no-such-pad", "--output", "json"]));
    assert_eq!(json["pads"].as_array().unwrap().len(), 0);
    // `filtered` distinguishes "nothing matched" from "no pads yet".
    assert_eq!(json["request"]["filtered"], serde_json::json!(true));

    // CSV drops the empty `pads` array entirely rather than emitting a column.
    let (headers, rows) =
        parse_csv(&fx.run(&["list", "--search", "zzz-no-such-pad", "--output", "csv"]));
    assert!(
        !headers.iter().any(|h| h.starts_with("pads.")),
        "empty array should contribute no CSV columns, got {headers:?}"
    );
    assert_eq!(rows.len(), 1, "CSV always emits exactly one row");
}

#[test]
fn singleton_and_multiple_results_stay_valid_in_every_format() {
    let fx = Fixture::new();

    fx.create("alpha");
    for mode in STRUCTURED_MODES {
        assert_parses(mode, &fx.run(&["list", "--output", mode]));
    }
    assert_eq!(
        titles_of(&parse_json(&fx.run(&["list", "--output", "json"]))).len(),
        1
    );

    fx.create("beta");
    fx.create("gamma");
    for mode in STRUCTURED_MODES {
        assert_parses(mode, &fx.run(&["list", "--output", mode]));
    }
    let titles = titles_of(&parse_json(&fx.run(&["list", "--output", "json"])));
    assert_eq!(titles.len(), 3);
    for expected in ["alpha", "beta", "gamma"] {
        assert!(titles.contains(&expected.to_string()), "missing {expected}");
    }
}

#[test]
fn nested_results_stay_valid_and_preserve_hierarchy() {
    let fx = Fixture::new();
    fx.create("parent");
    fx.create("child");

    // `move <src> <dest>`: nest "child" (1) under "parent" (2).
    fx.run(&["move", "1", "2"]);

    for mode in STRUCTURED_MODES {
        assert_parses(mode, &fx.run(&["list", "--output", mode]));
    }

    let json = parse_json(&fx.run(&["list", "--output", "json"]));
    let parent = json["pads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["pad"]["metadata"]["title"] == "parent")
        .expect("parent pad present at top level");

    let children = parent["children"].as_array().unwrap();
    assert_eq!(children.len(), 1, "parent should carry one child");
    assert_eq!(children[0]["pad"]["metadata"]["title"], "child");
    // The nesting edge is real data, not a rendered indent.
    assert_eq!(
        children[0]["pad"]["metadata"]["parent_id"],
        parent["pad"]["metadata"]["id"]
    );

    // YAML agrees with JSON on the whole nested structure.
    assert_eq!(parse_yaml(&fx.run(&["list", "--output", "yaml"])), json);
}

// ---------------------------------------------------------------------------
// Cross-format agreement.
// ---------------------------------------------------------------------------

/// JSON and YAML must be byte-for-byte equivalent as values; XML must carry the
/// same semantic leaves. CSV is excluded — its flattening is lossy by design and
/// is pinned separately.
#[test]
fn json_yaml_and_xml_agree_on_semantic_fields() {
    let fx = Fixture::new();
    fx.create("alpha");
    fx.create("beta");

    for cmd in [
        vec!["list"],
        vec!["view", "1"],
        vec!["path", "1"],
        vec!["uuid", "1"],
        vec!["doctor"],
    ] {
        let mut json_args = cmd.clone();
        json_args.extend_from_slice(&["--output", "json"]);
        let mut yaml_args = cmd.clone();
        yaml_args.extend_from_slice(&["--output", "yaml"]);

        let json = parse_json(&fx.run(&json_args));
        let yaml = parse_yaml(&fx.run(&yaml_args));
        assert_eq!(json, yaml, "JSON and YAML disagree for {cmd:?}");
    }

    // XML: same titles, same order as JSON.
    let json = parse_json(&fx.run(&["list", "--output", "json"]));
    let xml = parse_xml_leaves(&fx.run(&["list", "--output", "xml"]));
    let xml_titles: Vec<String> = xml
        .iter()
        .filter(|(tag, _)| tag == "title")
        .map(|(_, text)| text.clone())
        .collect();
    assert_eq!(
        xml_titles,
        titles_of(&json),
        "XML titles must match JSON titles"
    );
}

// ---------------------------------------------------------------------------
// CSV's flattening contract — deliberately pinned, because it is lossy.
// ---------------------------------------------------------------------------

/// CSV is standout's generic flattening of one result value, and it is *not*
/// row-per-pad. This test pins that contract so the loss is a documented decision
/// rather than a surprise:
///
/// 1. Exactly one header row and one data row, whatever the pad count.
/// 2. Columns are dotted paths (`pads.0.pad.metadata.title`); nesting deepens the
///    path (`pads.0.children.0...`) rather than adding rows.
/// 3. Empty arrays and nulls contribute no column at all, so the column set is
///    data-dependent and not a stable schema.
///
/// Consequence, documented for agents: use JSON or YAML for anything nested.
#[test]
fn csv_flattening_contract() {
    let fx = Fixture::new();
    fx.create("parent");
    fx.create("child");
    fx.run(&["move", "1", "2"]);

    let (headers, rows) = parse_csv(&fx.run(&["list", "--output", "csv"]));

    // (1) One row regardless of pad count.
    assert_eq!(rows.len(), 1, "CSV flattens the whole result into one row");

    // (2) Dotted paths, with nesting expressed in the column name.
    assert!(
        headers.iter().any(|h| h == "pads.0.pad.metadata.title"),
        "expected dotted-path columns, got {headers:?}"
    );
    assert!(
        headers
            .iter()
            .any(|h| h.starts_with("pads.0.children.0.") && h.ends_with(".title")),
        "nested pads must flatten into a deeper dotted path, got {headers:?}"
    );

    // Header/row widths agree — the row is well-formed for a CSV parser.
    assert_eq!(headers.len(), rows[0].len());

    // (3) Null/empty fields are omitted entirely rather than emitted as blanks.
    assert!(
        !headers.iter().any(|h| h == "pads.0.matches"),
        "null fields should contribute no CSV column, got {headers:?}"
    );
}

// ---------------------------------------------------------------------------
// Invariance: structured output is data, so the terminal must not influence it.
// ---------------------------------------------------------------------------

/// Structured bytes must not depend on terminal width, color forcing, or TTY-ness.
/// Human output legitimately does — that is what these modes exist to escape.
#[test]
fn structured_output_is_invariant_across_width_and_color() {
    let fx = Fixture::new();
    fx.create("a pad with a fairly long title that a narrow terminal would truncate");

    for mode in STRUCTURED_MODES {
        let args = ["list", "--output", mode];
        let narrow = fx.run_with_env(&args, &[("COLUMNS", "20")]);
        let wide = fx.run_with_env(&args, &[("COLUMNS", "400")]);
        let forced_color = fx.run_with_env(&args, &[("CLICOLOR_FORCE", "1"), ("FORCE_COLOR", "1")]);
        let no_color = fx.run_with_env(&args, &[("NO_COLOR", "1"), ("TERM", "dumb")]);

        assert_eq!(narrow, wide, "--output {mode} varies with terminal width");
        assert_eq!(
            narrow, forced_color,
            "--output {mode} varies with forced color"
        );
        assert_eq!(narrow, no_color, "--output {mode} varies with NO_COLOR");
    }

    // A short title is never elided, at any width.
    let fx2 = Fixture::new();
    fx2.create("short title");
    let narrow = parse_json(&fx2.run_with_env(&["list", "--output", "json"], &[("COLUMNS", "20")]));
    assert_eq!(titles_of(&narrow)[0], "short title");
}

/// `metadata.title` is capped at 60 chars (59 + `…`) by `padzapp`'s normalization
/// when the pad is written — a **data-level** rule, not a rendering one. It is easy
/// to mistake that `…` for the human renderer's width truncation leaking into data,
/// so this test pins the distinction: the cap is identical at every terminal width,
/// and `content` always carries the untruncated text.
#[test]
fn title_cap_is_a_data_rule_not_a_width_truncation() {
    let long = "a pad with a fairly long title that a narrow terminal would truncate";
    assert!(long.chars().count() > 60);

    let fx = Fixture::new();
    fx.create(long);

    let narrow = parse_json(&fx.run_with_env(&["list", "--output", "json"], &[("COLUMNS", "20")]));
    let wide = parse_json(&fx.run_with_env(&["list", "--output", "json"], &[("COLUMNS", "400")]));

    // Same cap at both widths — the ellipsis is data, not presentation.
    assert_eq!(narrow, wide);

    let title = &titles_of(&narrow)[0];
    assert_eq!(title.chars().count(), 60);
    assert!(title.ends_with('…'));

    // The full text is never lost: it lives in `content`.
    assert_eq!(narrow["pads"][0]["pad"]["content"], long);
}
