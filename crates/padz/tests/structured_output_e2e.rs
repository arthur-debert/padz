//! Focused process smoke for structured stdout.
//!
//! Structured command and format breadth lives in
//! `structured_output_harness.rs`, where the real application is exercised
//! without a child-process tax. This test retains the one fact only a process
//! can prove: the binary's final stdout/stderr/exit hop writes each structured
//! mode unchanged to a pipe.

#![allow(deprecated)]

use assert_cmd::cargo::cargo_bin;
use assert_cmd::Command;
use std::fs;
use tempfile::TempDir;

fn command(project: &std::path::Path, global: &std::path::Path) -> Command {
    let mut command = Command::new(cargo_bin("padz"));
    command.env("PADZ_GLOBAL_DATA", global).current_dir(project);
    command
}

#[test]
fn binary_writes_each_structured_mode_to_stdout() {
    let temp = TempDir::new().unwrap();
    let project = temp.path().join("project");
    let global = temp.path().join("global");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&global).unwrap();
    fs::create_dir(project.join(".git")).unwrap();

    command(&project, &global).arg("init").assert().success();
    command(&project, &global)
        .args(["create", "--no-editor", "stdout smoke"])
        .assert()
        .success();

    for mode in ["json", "yaml", "xml", "csv"] {
        let output = command(&project, &global)
            .args(["list", "--output", mode])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{mode}: process failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            output.stderr.is_empty(),
            "{mode}: successful structured output wrote stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).expect("structured stdout is UTF-8");

        match mode {
            "json" => {
                let value: serde_json::Value = serde_json::from_str(&stdout)
                    .unwrap_or_else(|error| panic!("not JSON: {error}\n{stdout}"));
                assert_eq!(value["pads"][0]["pad"]["metadata"]["title"], "stdout smoke");
            }
            "yaml" => {
                let value: serde_json::Value = serde_yaml::from_str(&stdout)
                    .unwrap_or_else(|error| panic!("not YAML: {error}\n{stdout}"));
                assert_eq!(value["pads"][0]["pad"]["metadata"]["title"], "stdout smoke");
            }
            "xml" => {
                let mut reader = quick_xml::Reader::from_str(&stdout);
                loop {
                    match reader.read_event() {
                        Ok(quick_xml::events::Event::Eof) => break,
                        Ok(_) => {}
                        Err(error) => panic!("not XML: {error}\n{stdout}"),
                    }
                }
            }
            "csv" => {
                let mut reader = csv::Reader::from_reader(stdout.as_bytes());
                reader.headers().expect("CSV header");
                for row in reader.records() {
                    row.expect("valid CSV row");
                }
            }
            _ => unreachable!(),
        }
    }
}
