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
//! The `data_override` parameter allows explicitly specifying the `.padz` data directory,
//! bypassing automatic scope detection. This is useful when:
//! - Working in a git worktree that should share data with the main repo
//! - Working in a temp directory for a project located elsewhere
//! - Explicitly pointing to a specific data store location
//!
//! When `data_override` is provided:
//! - The path is used directly as the project data directory
//! - Scope detection via [`find_project_root`] is skipped
//! - The scope defaults to `Scope::Project` (unless `-g` forces global)

use crate::api::{PadzApi, PadzPaths};
use crate::config::PadzConfig;
use crate::model::Scope;
use crate::store::fs::FileStore;
use directories::{BaseDirs, ProjectDirs};
use std::path::{Path, PathBuf};

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

/// Initialize the padz context with scope detection and store setup.
///
/// # Arguments
///
/// * `cwd` - The current working directory to start scope detection from
/// * `use_global` - If true, forces `Scope::Global` regardless of detection
/// * `data_override` - Optional explicit path to the `.padz` data directory.
///   When provided, bypasses automatic scope detection and uses this path directly.
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
/// // Use explicit data directory (e.g., for git worktrees)
/// let ctx = initialize(&cwd, false, Some(PathBuf::from("/path/to/project/.padz")));
/// ```
pub fn initialize(cwd: &Path, use_global: bool, data_override: Option<PathBuf>) -> PadzContext {
    // Determine project data directory:
    // 1. If data_override provided, use it directly
    // 2. Otherwise, try to find a project root with both .git and .padz
    // 3. Fallback to cwd/.padz
    let project_padz_dir = data_override.unwrap_or_else(|| {
        find_project_root(cwd)
            .map(|root| root.join(".padz"))
            .unwrap_or_else(|| cwd.join(".padz"))
    });

    let proj_dirs =
        ProjectDirs::from("com", "padz", "padz").expect("Could not determine config dir");
    let global_data_dir = proj_dirs.data_dir().to_path_buf();

    let scope = if use_global {
        Scope::Global
    } else {
        Scope::Project
    };

    let config_dir = match scope {
        Scope::Project => &project_padz_dir,
        Scope::Global => &global_data_dir,
    };
    let config = PadzConfig::load(config_dir).unwrap_or_default();
    let file_ext = config.get_file_ext().to_string();

    let store = FileStore::new(Some(project_padz_dir.clone()), global_data_dir.clone())
        .with_file_ext(&file_ext);
    let paths = PadzPaths {
        project: Some(project_padz_dir),
        global: global_data_dir,
    };
    let api = PadzApi::new(store, paths);

    PadzContext { api, scope, config }
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
    fn test_initialize_with_data_override_bypasses_detection() {
        // Setup: repo with .git and .padz
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        fs::create_dir(repo.join(".git")).unwrap();
        fs::create_dir(repo.join(".padz")).unwrap();

        // Create a separate override directory
        let override_dir = temp.path().join("custom-data");
        fs::create_dir_all(&override_dir).unwrap();

        // Initialize with override - should use override path, not the detected .padz
        let ctx = initialize(repo, false, Some(override_dir.clone()));

        // Verify the override path is used
        assert_eq!(ctx.api.paths().project, Some(override_dir));
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

        // Override directory
        let override_dir = temp.path().join("custom-data");
        fs::create_dir_all(&override_dir).unwrap();

        // Initialize with override AND global flag
        // The override still sets project path, but scope is Global
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
}
