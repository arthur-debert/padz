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

pub fn initialize(cwd: &Path, use_global: bool) -> PadzContext {
    // Try to find a project root with both .git and .padz
    let project_padz_dir = find_project_root(cwd)
        .map(|root| root.join(".padz"))
        .unwrap_or_else(|| cwd.join(".padz"));

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
}
