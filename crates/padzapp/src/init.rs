//! # Scopes: Project vs Global
//!
//! Command line tools typically operate in one of two modes:
//! 1. **Global**: Like a centralized database (e.g., standard notes apps).
//! 2. **Local**: Only in the current folder (e.g., `git`).
//!
//! For a developer-focused note tool, neither is sufficient. You want context-aware notes
//! ("TODOs for *this* project") but occasionally need global notes ("Grocery list")
//! without leaving your terminal.
//!
//! ## The Scopes
//!
//! - **Project**: Bound to a specific code repository. Data lives in `<repo_root>/.padz/`.
//! - **Global**: Bound to the user. Data lives in the OS-appropriate data directory
//!   (via the `directories` crate).
//!
//! ## Scope Detection Algorithm
//!
//! [`find_project_root`] implements git-aware scope detection:
//!
//! 1. Start at `CWD` (Current Working Directory).
//! 2. Check: Does this directory have BOTH `.git` AND `.padz`?
//! 3. **Match**: If yes, this is the Project Root.
//! 4. **No Match**: Move to parent directory.
//! 5. **Stop**: If we reach `HOME` or filesystem root, return `None`.
//!
//! **Why require both `.git` AND `.padz`?**
//! - If we only checked `.git`: We might accidentally use a parent repo in monorepos.
//! - If we only checked `.padz`: We lose the semantic binding to the "Project" concept.
//!
//! This means you must explicitly `padz init` in a repo to opt-in to project scope.
//!
//! ## Nested Repositories
//!
//! When working in nested repos (e.g., `parent-repo/child-repo`):
//! - If only `parent-repo` has `.padz`, starting from `child-repo` will find and use `parent-repo`
//! - If both have `.padz`, the innermost one (closest to cwd) wins
//! - If neither has `.padz`, uses `cwd/.padz` as default (may need `init`)
//!
//! ## Scope Resolution Flow
//!
//! The scope is resolved during [`initialize`]:
//! 1. If `-g` flag is present → Force `Scope::Global`.
//! 2. If `data_override` is provided → Use that path directly as project data directory.
//! 3. Otherwise → Run [`find_project_root`].
//!    - Found → `Scope::Project`.
//!    - Not Found → `Scope::Project` with `cwd/.padz` as path.
//!
//! ## Data Path Override
//!
//! The `data_override` parameter allows explicitly specifying the data directory,
//! bypassing automatic scope detection. This is useful when:
//! - Working in a git worktree that should share data with the main repo
//! - Working in a temp directory for a project located elsewhere
//! - Explicitly pointing to a specific data store location
//!
//! When `data_override` is provided:
//! - If the path ends with `.padz`, it's used directly as the data directory
//! - Otherwise, `.padz` is appended to the path (e.g., `/path/to/project` becomes `/path/to/project/.padz`)
//! - Scope detection via [`find_project_root`] is skipped
//! - The scope defaults to `Scope::Project` (unless `-g` forces global)

use crate::api::{PadzApi, PadzPaths};
use crate::config::PadzConfig;
use crate::error::PadzError;
use crate::model::Scope;
use crate::store::fs::FileStore;
use clapfig::{Clapfig, SearchMode, SearchPath};
use directories::{BaseDirs, ProjectDirs};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub struct PadzContext {
    pub api: PadzApi<FileStore>,
    pub scope: Scope,
    pub config: PadzConfig,
}

/// Find the project root by walking up from cwd looking for a directory
/// that has both .git and .padz. If a directory has .git but no .padz,
/// continue searching upward (to support nested repos where parent has padz).
/// Returns None if no matching directory is found before reaching home or root.
pub fn find_project_root(cwd: &Path) -> Option<PathBuf> {
    let home_dir = BaseDirs::new().map(|bd| bd.home_dir().to_path_buf());
    let mut current = cwd.to_path_buf();

    loop {
        let git_dir = current.join(".git");
        let padz_dir = current.join(".padz");

        // Found a repo with padz - use it
        if git_dir.exists() && padz_dir.exists() {
            return Some(current);
        }

        // Check stop conditions: reached home dir or volume root
        if let Some(ref home) = home_dir {
            if &current == home {
                return None;
            }
        }

        // Try to move up to parent
        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
            }
            _ => {
                // Reached filesystem root
                return None;
            }
        }
    }
}

/// Resolve a `.padz/link` file to the target data directory.
///
/// If `padz_dir` contains a `link` file, reads its contents as an absolute path
/// to a target project root, validates the target, and returns the resolved
/// `.padz` directory path.
///
/// Returns:
/// - `Ok(Some(path))` if a valid link was resolved
/// - `Ok(None)` if no link file exists
/// - `Err(...)` if the link file exists but is invalid (broken target, chained link)
pub fn resolve_link(padz_dir: &Path) -> crate::error::Result<Option<PathBuf>> {
    let link_file = padz_dir.join("link");
    if !link_file.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(&link_file)?;
    let target_str = raw.trim();
    if target_str.is_empty() {
        return Err(PadzError::Store(format!(
            "Link file at {} is empty",
            link_file.display()
        )));
    }

    let target = PathBuf::from(target_str);
    let target = target.canonicalize().map_err(|_| {
        PadzError::Store(format!(
            "Link target '{}' does not exist or is not accessible",
            target_str
        ))
    })?;

    // Determine the target .padz dir
    let target_padz = if target.file_name().is_some_and(|n| n == ".padz") {
        target
    } else {
        target.join(".padz")
    };

    // Validate target has been initialized (has active/ dir)
    if !target_padz.join("active").exists() {
        return Err(PadzError::Store(format!(
            "Link target '{}' has not been initialized (missing active/ directory). Run `padz init` there first.",
            target_padz.display()
        )));
    }

    // Reject chained links
    if target_padz.join("link").exists() {
        return Err(PadzError::Store(format!(
            "Link target '{}' is itself a link. Chained links are not supported.",
            target_padz.display()
        )));
    }

    Ok(Some(target_padz))
}

/// Initialize the padz context with scope detection and store setup.
///
/// # Arguments
///
/// * `cwd` - The current working directory to start scope detection from
/// * `use_global` - If true, forces `Scope::Global` regardless of detection
/// * `data_override` - Optional explicit path to the data directory.
///   When provided, bypasses automatic scope detection.
///   - If path ends with `.padz`, it's used as the data directory directly
///   - Otherwise, `.padz` is appended to the path
///
/// # Environment Variables
///
/// * `PADZ_GLOBAL_DATA` - If set, overrides the default global data directory.
///   This is primarily used for testing to isolate global state.
///
/// # Examples
///
/// ```ignore
/// // Normal initialization with automatic scope detection
/// let ctx = initialize(&cwd, false, None);
///
/// // Force global scope
/// let ctx = initialize(&cwd, true, None);
///
/// // Use explicit data directory - path ends with .padz, used directly
/// let ctx = initialize(&cwd, false, Some(PathBuf::from("/path/to/project/.padz")));
///
/// // Use explicit project directory - .padz is appended
/// let ctx = initialize(&cwd, false, Some(PathBuf::from("/path/to/project")));
/// ```
pub fn initialize(cwd: &Path, use_global: bool, data_override: Option<PathBuf>) -> PadzContext {
    // Determine project data directory:
    // 1. If data_override provided:
    //    - If it ends with ".padz", use it directly
    //    - Otherwise, append ".padz" to it
    // 2. Otherwise, try to find a project root with both .git and .padz
    // 3. Fallback to cwd/.padz
    let project_padz_dir = match data_override {
        Some(path) => {
            if path.file_name().is_some_and(|name| name == ".padz") {
                path
            } else {
                path.join(".padz")
            }
        }
        None => {
            let detected = find_project_root(cwd)
                .map(|root| root.join(".padz"))
                .unwrap_or_else(|| cwd.join(".padz"));
            // Follow .padz/link if present
            match resolve_link(&detected) {
                Ok(Some(linked)) => linked,
                _ => detected,
            }
        }
    };

    // Determine global data directory:
    // 1. Check PADZ_GLOBAL_DATA environment variable (primarily for testing)
    // 2. Fall back to OS-appropriate data directory via directories crate
    let global_data_dir = std::env::var("PADZ_GLOBAL_DATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let proj_dirs =
                ProjectDirs::from("com", "padz", "padz").expect("Could not determine config dir");
            proj_dirs.data_dir().to_path_buf()
        });

    let scope = if use_global {
        Scope::Global
    } else {
        Scope::Project
    };

    // Config search paths depend on scope:
    // - Global: only global dir (project config must not affect global operations)
    // - Project: both dirs merged (global provides defaults, project overrides)
    let config_search_paths = if use_global {
        vec![SearchPath::Path(global_data_dir.clone())]
    } else {
        vec![
            SearchPath::Path(global_data_dir.clone()),
            SearchPath::Path(project_padz_dir.clone()),
        ]
    };

    let config: PadzConfig = Clapfig::builder()
        .app_name("padz")
        .file_name("padz.toml")
        .search_paths(config_search_paths)
        .search_mode(SearchMode::Merge)
        .load()
        .unwrap_or_default();
    let file_ext = config.file_ext();

    // Migrate legacy flat layout to bucketed layout (if needed)
    migrate_if_needed(&project_padz_dir);
    migrate_if_needed(&global_data_dir);

    let store = FileStore::new_fs(Some(project_padz_dir.clone()), global_data_dir.clone())
        .with_file_ext(&file_ext);
    let paths = PadzPaths {
        project: Some(project_padz_dir),
        global: global_data_dir,
    };
    let api = PadzApi::new(store, paths);

    PadzContext { api, scope, config }
}

/// Migrates a legacy flat `.padz/` layout to the bucketed layout.
///
/// Legacy layout:
/// ```text
/// .padz/
///   data.json        # All pads (active + deleted via is_deleted flag)
///   pad-{uuid}.txt   # Content files
///   tags.json        # Tags (stays in place)
/// ```
///
/// Bucketed layout:
/// ```text
/// .padz/
///   tags.json        # Scope-level (shared)
///   active/
///     data.json      # Active pad metadata
///     pad-{uuid}.txt
///   archived/        # (empty after migration)
///     data.json
///   deleted/
///     data.json      # Deleted pad metadata
///     pad-{uuid}.txt
/// ```
///
/// Detection: `data.json` exists at root AND `active/` does NOT exist.
/// Idempotent: only runs if legacy layout detected.
fn migrate_if_needed(scope_root: &Path) {
    let legacy_data = scope_root.join("data.json");
    let active_dir = scope_root.join("active");

    // Only migrate if legacy data.json exists and active/ doesn't
    if !legacy_data.exists() || active_dir.exists() {
        return;
    }

    // Best-effort migration — log errors but don't crash
    if let Err(e) = migrate_flat_to_bucketed(scope_root) {
        eprintln!(
            "Warning: migration of {} failed: {}",
            scope_root.display(),
            e
        );
    }
}

fn migrate_flat_to_bucketed(scope_root: &Path) -> std::io::Result<()> {
    use std::collections::HashMap;
    use std::fs;

    let legacy_data_path = scope_root.join("data.json");
    let content = fs::read_to_string(&legacy_data_path)?;

    let entries: HashMap<Uuid, serde_json::Value> = serde_json::from_str(&content)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Partition entries by is_deleted flag
    let mut active_entries: HashMap<Uuid, serde_json::Value> = HashMap::new();
    let mut deleted_entries: HashMap<Uuid, serde_json::Value> = HashMap::new();

    for (id, mut value) in entries {
        let is_deleted = value
            .get("is_deleted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Strip legacy fields
        if let Some(obj) = value.as_object_mut() {
            obj.remove("is_deleted");
            obj.remove("deleted_at");
        }

        if is_deleted {
            deleted_entries.insert(id, value);
        } else {
            active_entries.insert(id, value);
        }
    }

    // Create bucket directories
    let active_dir = scope_root.join("active");
    let archived_dir = scope_root.join("archived");
    let deleted_dir = scope_root.join("deleted");
    fs::create_dir_all(&active_dir)?;
    fs::create_dir_all(&archived_dir)?;
    fs::create_dir_all(&deleted_dir)?;

    // Write bucket data.json files
    let active_json =
        serde_json::to_string_pretty(&active_entries).map_err(std::io::Error::other)?;
    fs::write(active_dir.join("data.json"), active_json)?;

    let deleted_json =
        serde_json::to_string_pretty(&deleted_entries).map_err(std::io::Error::other)?;
    fs::write(deleted_dir.join("data.json"), deleted_json)?;

    // Write empty archived data.json
    fs::write(archived_dir.join("data.json"), "{}")?;

    // Move content files to their respective bucket directories
    let all_active_ids: std::collections::HashSet<Uuid> = active_entries.keys().copied().collect();
    let all_deleted_ids: std::collections::HashSet<Uuid> =
        deleted_entries.keys().copied().collect();

    let dir_entries = fs::read_dir(scope_root)?;
    for entry in dir_entries {
        let entry = entry?;
        let path = entry.path();

        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.starts_with("pad-") {
            continue;
        }

        // Extract UUID from filename: pad-{uuid}.ext
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let uuid_part = stem.strip_prefix("pad-").unwrap_or("");
        let Ok(id) = Uuid::parse_str(uuid_part) else {
            continue;
        };

        let dest_dir = if all_deleted_ids.contains(&id) {
            &deleted_dir
        } else if all_active_ids.contains(&id) {
            &active_dir
        } else {
            // Orphan file — move to active (doctor will handle it)
            &active_dir
        };

        fs::rename(&path, dest_dir.join(name))?;
    }

    // Remove legacy data.json
    fs::remove_file(&legacy_data_path)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_find_project_root_with_git_and_padz() {
        // Setup: single repo with both .git and .padz
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir(root.join(".git")).unwrap();
        fs::create_dir(root.join(".padz")).unwrap();

        let result = find_project_root(root);
        assert_eq!(result, Some(root.to_path_buf()));
    }

    #[test]
    fn test_find_project_root_git_only_continues_up() {
        // Setup: child repo with .git only, parent with both .git and .padz
        let temp = TempDir::new().unwrap();
        let parent = temp.path();
        let child = parent.join("child-repo");

        fs::create_dir(&child).unwrap();
        fs::create_dir(parent.join(".git")).unwrap();
        fs::create_dir(parent.join(".padz")).unwrap();
        fs::create_dir(child.join(".git")).unwrap();
        // child has NO .padz

        let result = find_project_root(&child);
        assert_eq!(result, Some(parent.to_path_buf()));
    }

    #[test]
    fn test_find_project_root_nested_repos_child_has_padz() {
        // Setup: child repo with both .git and .padz should be used
        let temp = TempDir::new().unwrap();
        let parent = temp.path();
        let child = parent.join("child-repo");

        fs::create_dir(&child).unwrap();
        fs::create_dir(parent.join(".git")).unwrap();
        fs::create_dir(parent.join(".padz")).unwrap();
        fs::create_dir(child.join(".git")).unwrap();
        fs::create_dir(child.join(".padz")).unwrap();

        let result = find_project_root(&child);
        assert_eq!(result, Some(child.clone()));
    }

    #[test]
    fn test_find_project_root_deep_nested() {
        // Setup: deeply nested path finds grandparent with .git and .padz
        let temp = TempDir::new().unwrap();
        let grandparent = temp.path();
        let parent = grandparent.join("parent");
        let child = parent.join("child");

        fs::create_dir_all(&child).unwrap();
        fs::create_dir(grandparent.join(".git")).unwrap();
        fs::create_dir(grandparent.join(".padz")).unwrap();
        // parent and child have no .git or .padz

        let result = find_project_root(&child);
        assert_eq!(result, Some(grandparent.to_path_buf()));
    }

    #[test]
    fn test_find_project_root_no_git_no_padz() {
        // Setup: no .git or .padz anywhere in temp dir
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("some").join("deep").join("path");
        fs::create_dir_all(&dir).unwrap();

        let result = find_project_root(&dir);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_project_root_padz_only_no_git() {
        // Setup: .padz without .git should not match
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir(root.join(".padz")).unwrap();
        // No .git

        let result = find_project_root(root);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_project_root_multiple_nested_git_only_repos() {
        // Setup: multiple nested repos, only topmost has .padz
        // grandparent-repo/ (.git + .padz)
        //   parent-repo/ (.git only)
        //     child-repo/ (.git only)
        let temp = TempDir::new().unwrap();
        let grandparent = temp.path();
        let parent = grandparent.join("parent-repo");
        let child = parent.join("child-repo");

        fs::create_dir_all(&child).unwrap();
        fs::create_dir(grandparent.join(".git")).unwrap();
        fs::create_dir(grandparent.join(".padz")).unwrap();
        fs::create_dir(parent.join(".git")).unwrap();
        fs::create_dir(child.join(".git")).unwrap();

        // From child, should find grandparent
        let result = find_project_root(&child);
        assert_eq!(result, Some(grandparent.to_path_buf()));

        // From parent, should also find grandparent
        let result = find_project_root(&parent);
        assert_eq!(result, Some(grandparent.to_path_buf()));
    }

    // --- initialize() with data_override tests ---

    #[test]
    fn test_initialize_with_data_override_ending_in_padz() {
        // Setup: repo with .git and .padz
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        fs::create_dir(repo.join(".git")).unwrap();
        fs::create_dir(repo.join(".padz")).unwrap();

        // Create a separate override directory ending in .padz
        let override_dir = temp.path().join("custom-data").join(".padz");
        fs::create_dir_all(&override_dir).unwrap();

        // Initialize with override ending in .padz - should use it directly
        let ctx = initialize(repo, false, Some(override_dir.clone()));

        // Verify the override path is used directly (no .padz appended)
        assert_eq!(ctx.api.paths().project, Some(override_dir));
        assert_eq!(ctx.scope, crate::model::Scope::Project);
    }

    #[test]
    fn test_initialize_with_data_override_not_ending_in_padz() {
        // Setup: repo with .git and .padz
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        fs::create_dir(repo.join(".git")).unwrap();
        fs::create_dir(repo.join(".padz")).unwrap();

        // Create a separate override directory NOT ending in .padz
        let override_dir = temp.path().join("custom-project");
        fs::create_dir_all(&override_dir).unwrap();

        // Initialize with override - should append .padz
        let ctx = initialize(repo, false, Some(override_dir.clone()));

        // Verify .padz was appended
        assert_eq!(ctx.api.paths().project, Some(override_dir.join(".padz")));
        assert_eq!(ctx.scope, crate::model::Scope::Project);
    }

    #[test]
    fn test_initialize_without_override_uses_detection() {
        // Setup: repo with .git and .padz
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        fs::create_dir(repo.join(".git")).unwrap();
        fs::create_dir(repo.join(".padz")).unwrap();

        // Initialize without override - should use detected .padz
        let ctx = initialize(repo, false, None);

        // Verify the detected path is used
        assert_eq!(ctx.api.paths().project, Some(repo.join(".padz")));
        assert_eq!(ctx.scope, crate::model::Scope::Project);
    }

    #[test]
    fn test_initialize_data_override_with_global_flag() {
        // Setup: repo with .git and .padz
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        fs::create_dir(repo.join(".git")).unwrap();
        fs::create_dir(repo.join(".padz")).unwrap();

        // Override directory ending in .padz
        let override_dir = temp.path().join("custom-data").join(".padz");
        fs::create_dir_all(&override_dir).unwrap();

        // Initialize with override AND global flag
        // The override still sets project path, but scope is Global
        // Note: CLI prevents this combination, but library allows it
        let ctx = initialize(repo, true, Some(override_dir.clone()));

        assert_eq!(ctx.api.paths().project, Some(override_dir));
        assert_eq!(ctx.scope, crate::model::Scope::Global);
    }

    #[test]
    fn test_initialize_data_override_from_unrelated_directory() {
        // Use case: working in /tmp but want to use ~/projects/myproject/.padz
        let temp = TempDir::new().unwrap();

        // Create "project" with its .padz
        let project = temp.path().join("project");
        fs::create_dir_all(project.join(".padz")).unwrap();
        fs::create_dir(project.join(".git")).unwrap();

        // Create "workdir" - unrelated temp directory
        let workdir = temp.path().join("workdir");
        fs::create_dir_all(&workdir).unwrap();

        // Initialize from workdir, pointing to project's .padz
        let ctx = initialize(&workdir, false, Some(project.join(".padz")));

        // Should use project's .padz path
        assert_eq!(ctx.api.paths().project, Some(project.join(".padz")));
    }

    // --- Migration tests ---

    #[test]
    fn test_migration_flat_to_bucketed() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join(".padz");
        fs::create_dir_all(&root).unwrap();

        let id1 = Uuid::new_v4();
        let id2 = Uuid::new_v4();

        // Create legacy data.json with one active and one deleted pad
        let legacy_data = serde_json::json!({
            id1.to_string(): {
                "id": id1.to_string(),
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "is_pinned": false,
                "title": "Active Pad",
                "is_deleted": false
            },
            id2.to_string(): {
                "id": id2.to_string(),
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "is_pinned": false,
                "title": "Deleted Pad",
                "is_deleted": true,
                "deleted_at": "2024-01-02T00:00:00Z"
            }
        });
        fs::write(
            root.join("data.json"),
            serde_json::to_string_pretty(&legacy_data).unwrap(),
        )
        .unwrap();

        // Create content files
        fs::write(root.join(format!("pad-{}.txt", id1)), "Active content").unwrap();
        fs::write(root.join(format!("pad-{}.txt", id2)), "Deleted content").unwrap();

        // Create tags.json (should stay in place)
        fs::write(root.join("tags.json"), "[]").unwrap();

        // Run migration
        migrate_if_needed(&root);

        // Verify: legacy data.json removed
        assert!(!root.join("data.json").exists());

        // Verify: bucket directories created
        assert!(root.join("active").is_dir());
        assert!(root.join("archived").is_dir());
        assert!(root.join("deleted").is_dir());

        // Verify: active pad in active/
        let active_data: std::collections::HashMap<Uuid, serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(root.join("active/data.json")).unwrap())
                .unwrap();
        assert_eq!(active_data.len(), 1);
        assert!(active_data.contains_key(&id1));
        // Verify is_deleted stripped
        assert!(active_data[&id1].get("is_deleted").is_none());

        // Verify: deleted pad in deleted/
        let deleted_data: std::collections::HashMap<Uuid, serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(root.join("deleted/data.json")).unwrap())
                .unwrap();
        assert_eq!(deleted_data.len(), 1);
        assert!(deleted_data.contains_key(&id2));
        // Verify is_deleted and deleted_at stripped
        assert!(deleted_data[&id2].get("is_deleted").is_none());
        assert!(deleted_data[&id2].get("deleted_at").is_none());

        // Verify: content files moved
        assert!(root
            .join("active")
            .join(format!("pad-{}.txt", id1))
            .exists());
        assert!(root
            .join("deleted")
            .join(format!("pad-{}.txt", id2))
            .exists());
        assert!(!root.join(format!("pad-{}.txt", id1)).exists());
        assert!(!root.join(format!("pad-{}.txt", id2)).exists());

        // Verify: tags.json stays at root
        assert!(root.join("tags.json").exists());
    }

    #[test]
    fn test_migration_idempotent() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join(".padz");
        fs::create_dir_all(root.join("active")).unwrap();

        // Create a data.json at root (should NOT trigger migration since active/ exists)
        fs::write(root.join("data.json"), "{}").unwrap();

        migrate_if_needed(&root);

        // data.json should still exist (migration was skipped)
        assert!(root.join("data.json").exists());
    }

    #[test]
    fn test_migration_no_data_json() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join(".padz");
        fs::create_dir_all(&root).unwrap();

        // No data.json — nothing to migrate
        migrate_if_needed(&root);

        // No bucket directories should be created
        assert!(!root.join("active").exists());
    }

    #[test]
    fn test_migration_all_active() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join(".padz");
        fs::create_dir_all(&root).unwrap();

        let id = Uuid::new_v4();

        let legacy_data = serde_json::json!({
            id.to_string(): {
                "id": id.to_string(),
                "created_at": "2024-01-01T00:00:00Z",
                "updated_at": "2024-01-01T00:00:00Z",
                "is_pinned": false,
                "title": "Active"
            }
        });
        fs::write(
            root.join("data.json"),
            serde_json::to_string_pretty(&legacy_data).unwrap(),
        )
        .unwrap();
        fs::write(root.join(format!("pad-{}.txt", id)), "Content").unwrap();

        migrate_if_needed(&root);

        // All pads should be in active
        let active_data: std::collections::HashMap<Uuid, serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(root.join("active/data.json")).unwrap())
                .unwrap();
        assert_eq!(active_data.len(), 1);

        let deleted_data: std::collections::HashMap<Uuid, serde_json::Value> =
            serde_json::from_str(&fs::read_to_string(root.join("deleted/data.json")).unwrap())
                .unwrap();
        assert_eq!(deleted_data.len(), 0);
    }

    // --- resolve_link tests ---

    #[test]
    fn test_resolve_link_follows_link_file() {
        let temp = TempDir::new().unwrap();

        // Create target project with initialized .padz
        let target = temp.path().join("project-a");
        fs::create_dir_all(target.join(".padz").join("active")).unwrap();
        fs::create_dir_all(target.join(".padz").join("archived")).unwrap();
        fs::create_dir_all(target.join(".padz").join("deleted")).unwrap();

        // Create source .padz with link file
        let source_padz = temp.path().join("project-b").join(".padz");
        fs::create_dir_all(&source_padz).unwrap();
        fs::write(
            source_padz.join("link"),
            target.canonicalize().unwrap().to_str().unwrap(),
        )
        .unwrap();

        let result = resolve_link(&source_padz).unwrap();
        assert!(result.is_some());
        assert_eq!(
            result.unwrap(),
            target.canonicalize().unwrap().join(".padz")
        );
    }

    #[test]
    fn test_resolve_link_no_link_file() {
        let temp = TempDir::new().unwrap();
        let padz_dir = temp.path().join(".padz");
        fs::create_dir_all(&padz_dir).unwrap();

        let result = resolve_link(&padz_dir).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_link_broken_target() {
        let temp = TempDir::new().unwrap();
        let padz_dir = temp.path().join(".padz");
        fs::create_dir_all(&padz_dir).unwrap();
        fs::write(padz_dir.join("link"), "/nonexistent/path").unwrap();

        let result = resolve_link(&padz_dir);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn test_resolve_link_chained_link() {
        let temp = TempDir::new().unwrap();

        // Create target that is itself a link
        let target = temp.path().join("project-a");
        fs::create_dir_all(target.join(".padz").join("active")).unwrap();
        fs::write(target.join(".padz").join("link"), "/some/other/path").unwrap();

        // Create source with link to target
        let source_padz = temp.path().join("project-b").join(".padz");
        fs::create_dir_all(&source_padz).unwrap();
        fs::write(
            source_padz.join("link"),
            target.canonicalize().unwrap().to_str().unwrap(),
        )
        .unwrap();

        let result = resolve_link(&source_padz);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("itself a link"));
    }

    #[test]
    fn test_resolve_link_uninitialized_target() {
        let temp = TempDir::new().unwrap();

        // Create target without active/ dir (not initialized)
        let target = temp.path().join("project-a");
        fs::create_dir_all(target.join(".padz")).unwrap();

        let source_padz = temp.path().join("project-b").join(".padz");
        fs::create_dir_all(&source_padz).unwrap();
        fs::write(
            source_padz.join("link"),
            target.canonicalize().unwrap().to_str().unwrap(),
        )
        .unwrap();

        let result = resolve_link(&source_padz);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not been initialized"));
    }

    #[test]
    fn test_initialize_follows_link() {
        let temp = TempDir::new().unwrap();

        // Create project-a: fully initialized with .git and .padz
        let project_a = temp.path().join("project-a");
        fs::create_dir(project_a.join(".git")).unwrap_or_default();
        fs::create_dir_all(&project_a).unwrap();
        fs::create_dir(project_a.join(".git")).unwrap();
        fs::create_dir_all(project_a.join(".padz").join("active")).unwrap();
        fs::create_dir_all(project_a.join(".padz").join("archived")).unwrap();
        fs::create_dir_all(project_a.join(".padz").join("deleted")).unwrap();

        // Create project-b: has .git and .padz with link
        let project_b = temp.path().join("project-b");
        fs::create_dir_all(&project_b).unwrap();
        fs::create_dir(project_b.join(".git")).unwrap();
        fs::create_dir_all(project_b.join(".padz")).unwrap();
        fs::write(
            project_b.join(".padz").join("link"),
            project_a.canonicalize().unwrap().to_str().unwrap(),
        )
        .unwrap();

        // Initialize from project-b — should follow link to project-a
        let ctx = initialize(&project_b, false, None);
        assert_eq!(
            ctx.api.paths().project,
            Some(project_a.canonicalize().unwrap().join(".padz"))
        );
    }
}
