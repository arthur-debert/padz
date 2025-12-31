use assert_cmd::Command;

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
