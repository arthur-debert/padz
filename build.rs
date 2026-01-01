use std::process::Command;

fn main() {
    // Re-run if git HEAD changes
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/index");

    // Get the short commit hash
    let hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // Get the commit date in YYYY-MM-DD HH:MM format
    let commit_date = Command::new("git")
        .args(["log", "-1", "--format=%cd", "--date=format:%Y-%m-%d %H:%M"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_default();

    // Check if this is a clean release (no uncommitted changes, HEAD matches a tag)
    let is_dirty = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .ok()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    // Check if HEAD is exactly at a version tag
    let version = env!("CARGO_PKG_VERSION");
    let tag_at_head = Command::new("git")
        .args(["tag", "--points-at", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| {
            s.lines()
                .any(|tag| tag == format!("v{}", version) || tag == version)
        })
        .unwrap_or(false);

    let is_release = tag_at_head && !is_dirty;

    println!("cargo:rustc-env=GIT_HASH={}", hash);
    println!("cargo:rustc-env=GIT_COMMIT_DATE={}", commit_date);
    println!("cargo:rustc-env=IS_RELEASE={}", is_release);
}
