//! Structured-output contract at the in-process application seam.
//!
//! The command x format breadth belongs here: `TestHarness` drives the real
//! clap command, dispatch app, handlers, and Standout serializer without paying
//! for a child process per case. The process suite keeps only one smoke proving
//! that the final binary stdout hop remains connected.

mod support;

use standout::cli::{ExitStatus, RunErrorKind};
use standout::OutputMode;
use standout_test::{serial, TestHarness};
use support::Fixture;

#[derive(Clone, Copy)]
struct StructuredMode {
    name: &'static str,
    output: OutputMode,
}

const STRUCTURED_MODES: [StructuredMode; 4] = [
    StructuredMode {
        name: "json",
        output: OutputMode::Json,
    },
    StructuredMode {
        name: "yaml",
        output: OutputMode::Yaml,
    },
    StructuredMode {
        name: "xml",
        output: OutputMode::Xml,
    },
    StructuredMode {
        name: "csv",
        output: OutputMode::Csv,
    },
];

fn run(fx: &Fixture, args: &[&str], mode: StructuredMode) -> String {
    run_with(
        fx,
        args,
        mode,
        TestHarness::new()
            .cwd(fx.project())
            .env("NO_COLOR", "1")
            .interactive_stdin()
            .no_tty()
            .no_color()
            .terminal_width(80),
    )
}

fn run_with(fx: &Fixture, args: &[&str], mode: StructuredMode, harness: TestHarness) -> String {
    let (app, command) = fx.app(args);
    let result = harness
        .output_mode(mode.output)
        .run(&app, command, fx.argv(args));
    result.assert_success();
    result.stdout().to_string()
}

fn seed(fx: &Fixture, title: &str) {
    let state = fx.app_state();
    fx.seed_pad(&state, title, "");
}

fn parse_json(text: &str) -> serde_json::Value {
    serde_json::from_str(text)
        .unwrap_or_else(|error| panic!("not valid JSON: {error}\n---\n{text}\n---"))
}

fn parse_yaml(text: &str) -> serde_json::Value {
    let value: serde_yaml::Value = serde_yaml::from_str(text)
        .unwrap_or_else(|error| panic!("not valid YAML: {error}\n---\n{text}\n---"));
    serde_json::to_value(value).expect("YAML value is not JSON-representable")
}

fn parse_xml_leaves(text: &str) -> Vec<(String, String)> {
    use quick_xml::events::Event;

    let mut reader = quick_xml::Reader::from_str(text);
    reader.config_mut().trim_text(true);
    let mut leaves = Vec::new();
    let mut stack = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                stack.push(String::from_utf8_lossy(element.name().as_ref()).into_owned());
            }
            Ok(Event::End(_)) => {
                stack.pop();
            }
            Ok(Event::Empty(element)) => leaves.push((
                String::from_utf8_lossy(element.name().as_ref()).into_owned(),
                String::new(),
            )),
            Ok(Event::Text(value)) => {
                if let Some(tag) = stack.last() {
                    let value = value.unescape().unwrap_or_default().into_owned();
                    if !value.is_empty() {
                        leaves.push((tag.clone(), value));
                    }
                }
            }
            Ok(Event::Eof) => break,
            Ok(_) => {}
            Err(error) => panic!("not valid XML: {error}\n---\n{text}\n---"),
        }
    }

    leaves
}

fn parse_csv(text: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(text.as_bytes());
    let headers = reader
        .headers()
        .unwrap_or_else(|error| panic!("not valid CSV headers: {error}\n---\n{text}\n---"))
        .iter()
        .map(str::to_string)
        .collect();
    let rows = reader
        .records()
        .map(|row| {
            row.unwrap_or_else(|error| panic!("not valid CSV row: {error}\n---\n{text}\n---"))
                .iter()
                .map(str::to_string)
                .collect()
        })
        .collect();
    (headers, rows)
}

fn assert_parses(mode: StructuredMode, text: &str) {
    match mode.output {
        OutputMode::Json => {
            parse_json(text);
        }
        OutputMode::Yaml => {
            parse_yaml(text);
        }
        OutputMode::Xml => {
            parse_xml_leaves(text);
        }
        OutputMode::Csv => {
            parse_csv(text);
        }
        _ => unreachable!("{} is not a structured mode", mode.name),
    }
}

fn titles_of(value: &serde_json::Value) -> Vec<String> {
    fn walk(pads: &serde_json::Value, titles: &mut Vec<String>) {
        for pad in pads.as_array().into_iter().flatten() {
            if let Some(title) = pad["pad"]["metadata"]["title"].as_str() {
                titles.push(title.to_string());
            }
            walk(&pad["children"], titles);
        }
    }

    let mut titles = Vec::new();
    walk(&value["pads"], &mut titles);
    titles
}

#[test]
#[serial]
fn every_read_command_serializes_in_every_structured_mode() {
    let fx = Fixture::new();
    seed(&fx, "alpha");
    seed(&fx, "beta");

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

    for command in commands {
        for mode in STRUCTURED_MODES {
            let output = run(&fx, command, mode);
            assert_parses(mode, &output);
            assert!(
                !output.contains('\u{1b}'),
                "{command:?} in {} contains ANSI: {output}",
                mode.name
            );
        }
    }
}

#[test]
#[serial]
fn mutating_commands_serialize_in_every_structured_mode() {
    for mode in STRUCTURED_MODES {
        let fx = Fixture::new();
        seed(&fx, "alpha");

        for command in ["pin", "unpin", "complete", "reopen"] {
            let output = run(&fx, &[command, "1"], mode);
            assert_parses(mode, &output);
        }
    }
}

#[test]
#[serial]
fn create_serializes_through_output_and_stdin_seams_in_every_mode() {
    for mode in STRUCTURED_MODES {
        let fx = Fixture::new();
        let output = run_with(
            &fx,
            &["create", "--no-editor"],
            mode,
            TestHarness::new()
                .cwd(fx.project())
                .env("NO_COLOR", "1")
                .piped_stdin("fresh pad")
                .no_tty()
                .no_color()
                .terminal_width(80),
        );
        assert_parses(mode, &output);
    }
}

#[test]
#[serial]
fn explicit_output_flag_must_precede_create_trailing_title() {
    let fx = Fixture::new();
    let (app, command) = fx.app(&["create", "--no-editor", "fresh pad"]);
    let result = TestHarness::new().interactive_stdin().run(
        &app,
        command,
        fx.argv(&["create", "--no-editor", "--output", "json", "fresh pad"]),
    );
    result.assert_success();
    assert_parses(STRUCTURED_MODES[0], result.stdout());
    drop(result);

    let (app, command) = fx.app(&["create", "--no-editor", "swallowed"]);
    let result = TestHarness::new().interactive_stdin().run(
        &app,
        command,
        fx.argv(&["create", "--no-editor", "swallowed", "--output", "json"]),
    );
    result.assert_success();
    drop(result);

    let titles = titles_of(&parse_json(&run(&fx, &["list"], STRUCTURED_MODES[0])));
    assert!(titles.contains(&"swallowed --output json".to_string()));
}

#[test]
#[serial]
fn path_and_uuid_preserve_their_array_contract_in_every_mode() {
    let fx = Fixture::new();
    seed(&fx, "alpha");

    let path_json = parse_json(&run(&fx, &["path", "1"], STRUCTURED_MODES[0]));
    let path = path_json["paths"][0].as_str().expect("paths[0]");
    assert!(path.ends_with(".txt"));
    assert!(std::path::Path::new(path).exists());
    assert_eq!(
        parse_yaml(&run(&fx, &["path", "1"], STRUCTURED_MODES[1])),
        path_json
    );
    assert!(
        parse_xml_leaves(&run(&fx, &["path", "1"], STRUCTURED_MODES[2]))
            .iter()
            .any(|(tag, value)| tag == "paths" && value == path)
    );
    let (headers, rows) = parse_csv(&run(&fx, &["path", "1"], STRUCTURED_MODES[3]));
    assert_eq!(headers, vec!["paths.0"]);
    assert_eq!(rows[0], vec![path]);

    let uuid_json = parse_json(&run(&fx, &["uuid", "1"], STRUCTURED_MODES[0]));
    let uuid = uuid_json["uuids"][0].as_str().expect("uuids[0]");
    uuid::Uuid::parse_str(uuid).expect("canonical UUID");
    assert_eq!(
        parse_yaml(&run(&fx, &["uuid", "1"], STRUCTURED_MODES[1])),
        uuid_json
    );
    assert!(
        parse_xml_leaves(&run(&fx, &["uuid", "1"], STRUCTURED_MODES[2]))
            .iter()
            .any(|(tag, value)| tag == "uuids" && value == uuid)
    );
    let (headers, rows) = parse_csv(&run(&fx, &["uuid", "1"], STRUCTURED_MODES[3]));
    assert_eq!(headers, vec!["uuids.0"]);
    assert_eq!(rows[0], vec![uuid]);
}

#[test]
#[serial]
fn brackets_in_paths_are_not_treated_as_style_markup() {
    let fx = Fixture::with_project_name("pro[ject]");
    seed(&fx, "bracket pad");

    let value = parse_json(&run(&fx, &["path", "1"], STRUCTURED_MODES[0]));
    let path = value["paths"][0].as_str().expect("paths[0]");
    assert!(path.contains("pro[ject]"), "brackets changed in {path}");
    assert!(std::path::Path::new(path).exists());
}

#[test]
#[serial]
fn empty_singleton_multiple_and_nested_results_remain_structured() {
    let empty = Fixture::new();
    seed(&empty, "alpha");
    for mode in STRUCTURED_MODES {
        let output = run(&empty, &["list", "--search", "zzz-no-match"], mode);
        assert_parses(mode, &output);
    }
    let json = parse_json(&run(
        &empty,
        &["list", "--search", "zzz-no-match"],
        STRUCTURED_MODES[0],
    ));
    assert_eq!(json["pads"], serde_json::json!([]));
    assert_eq!(json["request"]["filtered"], true);
    let (headers, rows) = parse_csv(&run(
        &empty,
        &["list", "--search", "zzz-no-match"],
        STRUCTURED_MODES[3],
    ));
    assert!(!headers.iter().any(|header| header.starts_with("pads.")));
    assert_eq!(rows.len(), 1);

    let cardinality = Fixture::new();
    seed(&cardinality, "alpha");
    for mode in STRUCTURED_MODES {
        assert_parses(mode, &run(&cardinality, &["list"], mode));
    }
    assert_eq!(
        titles_of(&parse_json(&run(
            &cardinality,
            &["list"],
            STRUCTURED_MODES[0]
        )))
        .len(),
        1
    );
    seed(&cardinality, "beta");
    seed(&cardinality, "gamma");
    for mode in STRUCTURED_MODES {
        assert_parses(mode, &run(&cardinality, &["list"], mode));
    }
    let titles = titles_of(&parse_json(&run(
        &cardinality,
        &["list"],
        STRUCTURED_MODES[0],
    )));
    assert_eq!(titles.len(), 3);
    for expected in ["alpha", "beta", "gamma"] {
        assert!(titles.contains(&expected.to_string()));
    }

    let nested = Fixture::new();
    let state = nested.app_state();
    nested.seed_pad(&state, "parent", "");
    nested.seed_child(&state, "1", "child", "");
    drop(state);
    for mode in STRUCTURED_MODES {
        assert_parses(mode, &run(&nested, &["list"], mode));
    }
    let json = parse_json(&run(&nested, &["list"], STRUCTURED_MODES[0]));
    assert_eq!(
        json["pads"][0]["children"][0]["pad"]["metadata"]["title"],
        "child"
    );
    assert_eq!(
        json["pads"][0]["children"][0]["pad"]["metadata"]["parent_id"],
        json["pads"][0]["pad"]["metadata"]["id"]
    );
    assert_eq!(
        parse_yaml(&run(&nested, &["list"], STRUCTURED_MODES[1])),
        json
    );
}

#[test]
#[serial]
fn structured_modes_agree_and_csv_flattening_remains_explicitly_lossy() {
    let fx = Fixture::new();
    seed(&fx, "alpha");
    seed(&fx, "beta");

    for command in [
        &["list"][..],
        &["view", "1"],
        &["path", "1"],
        &["uuid", "1"],
        &["doctor"],
    ] {
        let json = parse_json(&run(&fx, command, STRUCTURED_MODES[0]));
        let yaml = parse_yaml(&run(&fx, command, STRUCTURED_MODES[1]));
        assert_eq!(json, yaml, "JSON and YAML disagree for {command:?}");
    }

    let json = parse_json(&run(&fx, &["list"], STRUCTURED_MODES[0]));
    let xml_titles: Vec<String> = parse_xml_leaves(&run(&fx, &["list"], STRUCTURED_MODES[2]))
        .into_iter()
        .filter(|(tag, _)| tag == "title")
        .map(|(_, value)| value)
        .collect();
    assert_eq!(xml_titles, titles_of(&json));

    let nested = Fixture::new();
    let state = nested.app_state();
    nested.seed_pad(&state, "parent", "");
    nested.seed_child(&state, "1", "child", "");
    drop(state);
    let (headers, rows) = parse_csv(&run(&nested, &["list"], STRUCTURED_MODES[3]));
    assert_eq!(rows.len(), 1);
    assert_eq!(headers.len(), rows[0].len());
    assert!(headers
        .iter()
        .any(|header| header == "pads.0.pad.metadata.title"));
    assert!(headers
        .iter()
        .any(|header| header.starts_with("pads.0.children.0.") && header.ends_with(".title")));
    assert!(!headers.iter().any(|header| header == "pads.0.matches"));
}

#[test]
#[serial]
fn structured_output_has_no_human_artifacts_or_template_only_fields() {
    let fx = Fixture::new();
    seed(&fx, "notes on indent, time_ago and cols");

    let (app, command) = fx.app(&["list"]);
    let human = TestHarness::new().output_mode(OutputMode::TermDebug).run(
        &app,
        command,
        fx.argv(&["list"]),
    );
    human.assert_success();
    let human = human.stdout().to_string();

    for mode in STRUCTURED_MODES {
        let output = run(&fx, &["list"], mode);
        assert_parses(mode, &output);
        assert_ne!(output.trim(), human.trim());
        for artifact in ["\u{1b}", "⏲", "⚲", "[dim]", "[bold]", "[/]", "[muted]"] {
            assert!(
                !output.contains(artifact),
                "{} leaked human artifact {artifact:?}: {output}",
                mode.name
            );
        }
    }

    let json = parse_json(&run(&fx, &["list"], STRUCTURED_MODES[0]));
    let mut top: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    top.sort_unstable();
    assert_eq!(top, vec!["pads", "request"]);

    fn collect_keys(value: &serde_json::Value, keys: &mut std::collections::BTreeSet<String>) {
        match value {
            serde_json::Value::Object(fields) => {
                for (key, child) in fields {
                    keys.insert(key.clone());
                    collect_keys(child, keys);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    collect_keys(item, keys);
                }
            }
            _ => {}
        }
    }
    let mut keys = std::collections::BTreeSet::new();
    collect_keys(&json, &mut keys);
    for forbidden in [
        "rows",
        "line_width",
        "title_width",
        "indent",
        "time_ago",
        "status_icon",
        "left_pin",
        "columns",
        "cols",
    ] {
        assert!(!keys.contains(forbidden), "template key {forbidden} leaked");
    }
}

#[test]
#[serial]
fn warning_and_failure_paths_use_structured_and_typed_harness_seams() {
    for mode in STRUCTURED_MODES {
        let fx = Fixture::new();
        let source = fx.root().join("bad.md");
        std::fs::write(
            &source,
            "---\npadz.status: NotAThing\n---\n\nImported title\n\nBody",
        )
        .unwrap();
        let source = source.to_str().unwrap();
        let output = run(&fx, &["import", source], mode);
        assert_parses(mode, &output);

        let (app, command) = fx.app(&["view", "999"]);
        let failure = TestHarness::new()
            .cwd(fx.project())
            .env("NO_COLOR", "1")
            .interactive_stdin()
            .no_tty()
            .no_color()
            .terminal_width(80)
            .output_mode(mode.output)
            .run(&app, command, fx.argv(&["view", "999"]));
        failure.assert_error();
        failure.assert_exit_status(ExitStatus::FAILURE);
        failure.assert_error_kind(RunErrorKind::Handler);
    }
}

#[test]
#[serial]
fn structured_bytes_ignore_width_tty_color_and_environment() {
    let fx = Fixture::new();
    seed(
        &fx,
        "a pad with a fairly long title that a narrow terminal would truncate",
    );

    for mode in STRUCTURED_MODES {
        let narrow = run_with(
            &fx,
            &["list"],
            mode,
            TestHarness::new()
                .cwd(fx.project())
                .env("COLUMNS", "20")
                .env("NO_COLOR", "1")
                .interactive_stdin()
                .no_tty()
                .no_color()
                .terminal_width(20),
        );
        let wide = run_with(
            &fx,
            &["list"],
            mode,
            TestHarness::new()
                .cwd(fx.project())
                .env("COLUMNS", "400")
                .env_remove("NO_COLOR")
                .interactive_stdin()
                .is_tty()
                .with_color()
                .terminal_width(400),
        );
        assert_eq!(narrow, wide, "{} varies with terminal seams", mode.name);
    }

    let narrow = parse_json(&run_with(
        &fx,
        &["list"],
        STRUCTURED_MODES[0],
        TestHarness::new().no_color().terminal_width(20),
    ));
    let wide = parse_json(&run_with(
        &fx,
        &["list"],
        STRUCTURED_MODES[0],
        TestHarness::new().with_color().terminal_width(400),
    ));
    assert_eq!(narrow, wide);
    let title = &titles_of(&narrow)[0];
    assert_eq!(title.chars().count(), 60);
    assert!(title.ends_with('…'));
    assert_eq!(
        narrow["pads"][0]["pad"]["content"],
        "a pad with a fairly long title that a narrow terminal would truncate"
    );
}

/// This file uses process-global TestHarness overrides, so every behavioral
/// test above must be serial. The guard itself does not touch the harness.
#[test]
fn every_structured_harness_test_is_serial() {
    let source = include_str!("structured_output_harness.rs");
    let lines: Vec<&str> = source.lines().collect();
    let mut offenders = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        if line.trim() != "#[test]" {
            continue;
        }
        let attrs: Vec<&str> = lines[index + 1..]
            .iter()
            .take_while(|line| line.trim_start().starts_with('#'))
            .copied()
            .collect();
        let function = lines[index + 1..]
            .iter()
            .find(|line| line.trim_start().starts_with("fn "))
            .copied()
            .unwrap_or("<unknown>");
        if function.contains("fn every_structured_harness_test_is_serial") {
            continue;
        }
        if !attrs.iter().any(|attr| attr.trim() == "#[serial]") {
            offenders.push(function.trim().to_string());
        }
    }

    assert!(
        offenders.is_empty(),
        "these structured harness tests are missing #[serial]: {offenders:#?}"
    );
}
