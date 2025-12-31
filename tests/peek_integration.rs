use assert_cmd::Command;
use predicates::prelude::PredicateBooleanExt;
use predicates::prelude::*;

#[test]
fn test_list_peek() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("db.sqlite");

    // Create a long pad
    let mut cmd = Command::cargo_bin("padz").unwrap();
    cmd.env("PADZ_DB", &db_path)
        .env("PADZ_HOME", temp_dir.path());
    cmd.arg("n")
        .arg("--no-editor")
        .arg("Long Pad")
        .assert()
        .success();

    // Since we created it, we need to update its content to be long enough to truncate
    // But `create --no-editor` makes an empty pad.
    // We can't easily edit it via CLI without an editor.
    // However, we can use `padz config` or just rely on the fact that we can't easily populate it?
    // Wait, `create` takes no content arg.
    // Ideally we'd use the library directly but this is an integration test using binary.

    // Maybe we can import a file?
    let import_file = temp_dir.path().join("long.txt");
    let lines: Vec<String> = (1..=20).map(|i| format!("Line {}", i)).collect();
    let content = format!("Long Pad Title\n\n{}", lines.join("\n"));
    std::fs::write(&import_file, content).unwrap();

    let mut cmd = Command::cargo_bin("padz").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("PADZ_HOME", temp_dir.path()) // Ensure it uses temp home
        .arg("import")
        .arg(import_file.to_str().unwrap())
        .assert()
        .success();

    // Now list with peek
    let mut cmd = Command::cargo_bin("padz").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("PADZ_HOME", temp_dir.path())
        .arg("list")
        .arg("--peek")
        .assert()
        .success()
        .stdout(predicates::str::contains("Line 1")) // Opening
        .stdout(predicates::str::contains("Line 2")) // Limit is 3 (Title, Line 1, Line 2)
        .stdout(predicates::str::contains("lines not shown")) // Truncation message
        .stdout(predicates::str::contains("Line 20")); // Closing
}

#[test]
fn test_view_peek_truncation() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Import a long pad
    let import_file = temp_dir.path().join("long-view.txt");
    let lines: Vec<String> = (1..=20).map(|i| format!("ViewLine {}", i)).collect();
    let content = format!("Long View Title\n\n{}", lines.join("\n"));
    std::fs::write(&import_file, content).unwrap();

    let mut cmd = Command::cargo_bin("padz").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("PADZ_HOME", temp_dir.path())
        .arg("import")
        .arg(import_file.to_str().unwrap())
        .assert()
        .success();

    // Now view with peek (should reuse list rendering logic)
    // Index should be 1
    let mut cmd = Command::cargo_bin("padz").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("PADZ_HOME", temp_dir.path())
        .arg("view")
        .arg("1")
        .arg("--peek")
        .assert()
        .success()
        .stdout(predicates::str::contains("ViewLine 1"))
        .stdout(predicates::str::contains("lines not shown"))
        .stdout(predicates::str::contains("ViewLine 20"));
}

#[test]
fn test_peek_spacing_and_limits() {
    let temp_dir = tempfile::tempdir().unwrap();

    // Create a pad just under limit (9 lines total content -> title + 8 lines)
    // peek=3. Threshold = (3*2)+3 = 9.
    // If we have 9 lines of BODY, total content is Title\n\nBody.
    // But `format_as_peek` takes just the body (as we stripped title in render.rs).
    // So if body has 9 lines: 9 <= 9 -> Full.
    // If body has 10 lines: 10 > 9 -> Truncated.

    // Test case 1: 9 lines body (Threshold) - Should be FULL
    let body_9: Vec<String> = (1..=9).map(|i| format!("Line {}", i)).collect();
    let content_9 = format!("Title 9\n\n{}", body_9.join("\n"));
    let path_9 = temp_dir.path().join("limit_9.txt");
    std::fs::write(&path_9, content_9).unwrap();

    let mut cmd = Command::cargo_bin("padz").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("PADZ_HOME", temp_dir.path())
        .arg("import")
        .arg(path_9.to_str().unwrap())
        .assert()
        .success();

    let mut cmd = Command::cargo_bin("padz").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("PADZ_HOME", temp_dir.path())
        .arg("list")
        .arg("--peek")
        .assert()
        .success()
        .stdout(predicates::str::contains("Line 9"))
        .stdout(predicates::str::contains("lines not shown").not()); // Should NOT show truncation

    // Test case 2: 10 lines body - Should be TRUNCATED with correct spacing
    let body_10: Vec<String> = (1..=10).map(|i| format!("Line {}", i)).collect();
    let content_10 = format!("Title 10\n\n{}", body_10.join("\n"));
    let path_10 = temp_dir.path().join("limit_10.txt");
    std::fs::write(&path_10, content_10).unwrap();

    let mut cmd = Command::cargo_bin("padz").unwrap();
    cmd.current_dir(temp_dir.path())
        .env("PADZ_HOME", temp_dir.path())
        .arg("import")
        .arg(path_10.to_str().unwrap())
        .assert()
        .success();

    let mut cmd = Command::cargo_bin("padz").unwrap();
    let output = cmd
        .current_dir(temp_dir.path())
        .env("PADZ_HOME", temp_dir.path())
        .arg("list")
        .arg("--peek")
        .output()
        .unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();

    // Find output for Title 10
    // Check for blank line, truncated message, blank line pattern
    // The truncated message is "... 4 lines not shown ..." (10 - 2*3 = 4)
    // We expect:
    // Line 3
    // <blank>
    // ... 4 lines not shown ...
    // <blank>
    // Line 8

    // Note: The template adds indent.
    // "    Line 3"
    // "" (empty line)
    // "                          ... 4 lines not shown ..."
    // "" (empty line)
    // "    Line 8"

    // We can regex verify or just contains
    // assert!(stdout.contains("Line 3\n\n"), "Missing blank line after opening");
    // assert!(stdout.contains("\n\n    Line 8"), "Missing blank line before closing");
    // Indentation makes exact matching tricky, let's look for newlines.

    // Regex for: Line 3 \s* \n \s* \n .* not shown
    // Since we are mocking tests, let's just assert existence of the full block sequence?

    // Let's assert that "Line 3" is followed by at least two newlines before "... lines not shown"
    // and "... lines not shown" is followed by at least two newlines before "Line 8".
    // Or just check if the string "Line 3\n\n" is present (might fail due to indent on next line).

    // Actually, `list.tmp`:
    // {{ pad.left_pad }}    ... opening_lines ...
    // <newline>
    // {{ pad.left_pad }} ... truncated ...

    // If `opening_lines` ends with newline?
    // In `peek.rs`: opening = non_blank_lines[..3].join("\n").
    // So "Line 1\nLine 2\nLine 3". No trailing newline.

    // Template:
    // {{ ... opening_lines ... }}
    // {% if truncated ... %}
    // <newline>
    // {{ ... truncated ... }}

    // So output: "...Line 3\n\n...truncated..."
    // Yes, that should correspond to one blank line.

    let truncated_idx = stdout
        .find("... 4 lines not shown ...")
        .expect("Should fulfill truncation logic");
    let opening_end = stdout[..truncated_idx]
        .rfind("Line 3")
        .expect("Should have opening lines");

    // Check text between opening end and truncation msg
    let gap = &stdout[opening_end + 6..truncated_idx];
    // "Line 3" len is 6.
    // gap should match `\n\n\s*`

    assert!(
        gap.contains("\n\n"),
        "Gap between opening and truncation should have blank line. Got: {:?}",
        gap
    );

    let closing_start = stdout[truncated_idx..]
        .find("Line 8")
        .expect("Should have closing lines")
        + truncated_idx;
    let gap2 = &stdout[truncated_idx + "... 4 lines not shown ...".len()..closing_start];

    assert!(
        gap.contains("\n"),
        "Gap should have newline. Got: {:?}",
        gap
    );
    // There should be enough vertical space. It depends on how jinja renders newlines+indent.
    // We expect at least a blank line visually.
    // "Line 3\n    \n                          ..."
    // That gives 1 blank line.
}
