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
//! - **Project**: Bound to a specific `.padz/` directory. Data lives in `<project_root>/.padz/`.
//! - **Global**: Bound to the user. Data lives in the OS-appropriate data directory
//!   (via the `directories` crate).
//!
//! ## Two Discovery Modes
//!
//! Padz uses **two different discovery algorithms** depending on what the command does:
//!
//! ### Read discovery — used by every command
//!
//! [`find_padz_root`] walks up from cwd looking for `.padz` alone:
//!
//! 1. Start at `CWD`.
//! 2. If `current/.padz` exists → this is the project root.
//! 3. Otherwise move to the parent directory.
//! 4. Stop at `$HOME` or the filesystem root; return `None`.
//!
//! If a `.padz` is found, it is used (resolving any `link` file). Otherwise the read
//! falls back to the global store.
//!
//! ### Auto-init discovery — used only by write commands (create, import)
//!
//! If read discovery finds nothing and the command is creating a new pad, padz tries
//! to place that pad inside a project rather than silently dropping it into global.
//! It runs [`find_git_root`] — the same upward walk, but looking for `.git` — and, if
//! a git repo is found, creates a fresh `.padz/` at the git root and uses it.
//!
//! **Summary of the fallback chain for writes:**
//!
//! 1. `.padz` found upward → use it.
//! 2. Else `.git` found upward → auto-init `.padz` at the git root, use it.
//! 3. Else → global.
//!
//! Reads never auto-init; they simply fall through to step 3 if step 1 misses.
//!
//! ## Explicit `padz init`
//!
//! `padz init` is user intent and is never blocked: it always creates `.padz/` at
//! cwd, regardless of whether the directory is a git repo, already inside another
//! project, or anywhere else. The CLI layer handles this by forcing `cwd` as the
//! data override for plain-init, bypassing the discovery logic entirely.
//!
//! ## Nested Repositories
//!
//! With the `.padz`-only read rule, nested behavior is straightforward:
//! - The innermost `.padz` (closest to cwd on the upward walk) wins.
//! - Subprojects inherit their parent's store unless they run `padz init` themselves.
//! - A `.padz/link` file transparently redirects to another project's store.
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
//! - Both discovery algorithms are skipped
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

/// Materialize the padz store layout (`active/`, `archived/`, `deleted/`) at
/// `padz_dir`. Idempotent — safe to call on an existing store. Shared between
/// explicit `padz init` (in `commands::init`) and the auto-init-on-write path
/// in [`initialize`]; callers decide how to react to failure.
pub fn create_bucket_layout(padz_dir: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(padz_dir.join("active"))?;
    std::fs::create_dir_all(padz_dir.join("archived"))?;
    std::fs::create_dir_all(padz_dir.join("deleted"))?;
    Ok(())
}

/// Walk upward from `cwd` looking for a directory that contains a `.padz/`
/// subdirectory.
///
/// This is the **read** discovery function: every command uses it to locate an
/// existing store, regardless of whether the enclosing directory is a git repo.
/// `.padz` must be a directory; a stray regular file with that name is ignored
/// so it cannot masquerade as a store and trip later I/O.
/// Stops at `$HOME` or the filesystem root and returns `None`.
pub fn find_padz_root(cwd: &Path) -> Option<PathBuf> {
    walk_up_matching(cwd, |dir| dir.join(".padz").is_dir())
}

/// Walk upward from `cwd` looking for a directory that contains a `.git` entry
/// (directory or file).
///
/// This is the **auto-init** discovery function: used by write commands to decide
/// where to create a new `.padz/` when none is found upward. Accepts `.git` as
/// either a directory (normal repo) or a regular file (git worktrees and
/// submodules store the real gitdir elsewhere and leave a `.git` file pointer).
/// Stops at `$HOME` or the filesystem root and returns `None`.
pub fn find_git_root(cwd: &Path) -> Option<PathBuf> {
    walk_up_matching(cwd, |dir| dir.join(".git").exists())
}

/// Shared upward-walk helper. Returns the first ancestor (including `cwd`
/// itself) for which `matches` returns true. Stops at `$HOME` or the
/// filesystem root.
fn walk_up_matching<F>(cwd: &Path, matches: F) -> Option<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    let home_dir = BaseDirs::new().map(|bd| bd.home_dir().to_path_buf());
    let mut current = cwd.to_path_buf();

    loop {
        if matches(&current) {
            return Some(current);
        }

        if let Some(ref home) = home_dir {
            if &current == home {
                return None;
            }
        }

        match current.parent() {
            Some(parent) if parent != current => {
                current = parent.to_path_buf();
            }
            _ => return None,
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
/// * `auto_init_for_write` - If true and read discovery finds no `.padz`, fall
///   through to `find_git_root` and create `.padz/` at the git root before
///   returning `Scope::Project`. Only write commands (`create`, `import`) should
///   pass `true`; reads pass `false` and fall back to global cleanly.
///
/// # Environment Variables
///
/// * `PADZ_GLOBAL_DATA` - If set, overrides the default global data directory.
///   This is primarily used for testing to isolate global state.
///
/// # Errors
///
/// Returns `Err` when the user's intent is clearly project-scoped but the
/// project store is unusable:
/// - A `.padz/link` file exists but resolves to a broken, uninitialized, or
///   chained target. Silently falling back to the local `.padz` would send
///   writes to a different store than the user configured.
/// - Auto-init was requested (`auto_init_for_write = true`) and a git root
///   was found, but creating `.padz/` at that root failed. Silently going
///   to Global would drop the new pad somewhere the user almost certainly
///   did not mean; making the caller abort is the safer default.
///
/// All other paths (no `.padz` upward on a read, no `.git` on a write, etc.)
/// fall back to `Scope::Global` successfully.
///
/// # Examples
///
/// ```ignore
/// // Read path: discover .padz upward, else global
/// let ctx = initialize(&cwd, false, None, false)?;
///
/// // Write path: discover .padz upward; if none, auto-init at git root; else global
/// let ctx = initialize(&cwd, false, None, true)?;
///
/// // Force global scope
/// let ctx = initialize(&cwd, true, None, false)?;
///
/// // Use explicit data directory - path ends with .padz, used directly
/// let ctx = initialize(&cwd, false, Some(PathBuf::from("/path/to/project/.padz")), false)?;
///
/// // Use explicit project directory - .padz is appended
/// let ctx = initialize(&cwd, false, Some(PathBuf::from("/path/to/project")), false)?;
/// ```
pub fn initialize(
    cwd: &Path,
    use_global: bool,
    data_override: Option<PathBuf>,
    auto_init_for_write: bool,
) -> crate::error::Result<PadzContext> {
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

    // Determine project data directory and scope:
    // 1. If use_global → Global scope, no project dir
    // 2. If data_override provided → Project scope with explicit path
    // 3. find_padz_root found something → Project scope (follow link if present,
    //    propagate link-resolution errors rather than silently using the local path)
    // 4. Else if auto_init_for_write and find_git_root found something → create
    //    .padz at that git root and use it (Project scope), propagating bucket-
    //    creation errors rather than silently dropping the pad into global
    // 5. Else → fall back to Global scope
    let (project_padz_dir, scope) = if use_global {
        (None, Scope::Global)
    } else {
        match data_override {
            Some(path) => {
                let dir = if path.file_name().is_some_and(|name| name == ".padz") {
                    path
                } else {
                    path.join(".padz")
                };
                (Some(dir), Scope::Project)
            }
            None => match find_padz_root(cwd) {
                Some(root) => {
                    let detected = root.join(".padz");
                    // Follow .padz/link if present. A broken/uninitialized/
                    // chained link is the user's declared intent going wrong;
                    // bubble the error up so it is visible instead of silently
                    // operating on the local (unlinked) directory, which might
                    // be a different store.
                    let resolved = match resolve_link(&detected)? {
                        Some(linked) => linked,
                        None => detected,
                    };
                    (Some(resolved), Scope::Project)
                }
                None if auto_init_for_write => {
                    // Write path: try to auto-init a project store at the
                    // enclosing git root. If no git root is found, fall back
                    // to Global (the user isn't clearly inside a project). If
                    // a git root IS found but layout creation fails, surface
                    // the error — the write would otherwise silently land in
                    // Global despite the user sitting in a git repo.
                    match find_git_root(cwd) {
                        Some(git_root) => {
                            let new_padz = git_root.join(".padz");
                            create_bucket_layout(&new_padz).map_err(|err| {
                                PadzError::Store(format!(
                                    "could not auto-init padz store at {}: {}. \
                                     Run `padz init` there (or `-g` to force global) to proceed.",
                                    new_padz.display(),
                                    err
                                ))
                            })?;
                            (Some(new_padz), Scope::Project)
                        }
                        None => (None, Scope::Global),
                    }
                }
                None => (None, Scope::Global),
            },
        }
    };

    // Config search paths depend on scope:
    // - Global: only global dir (project config must not affect global operations)
    // - Project: both dirs merged (global provides defaults, project overrides)
    let mut config_search_paths = vec![SearchPath::Path(global_data_dir.clone())];
    if let Some(ref project_dir) = project_padz_dir {
        config_search_paths.push(SearchPath::Path(project_dir.clone()));
    }

    let config: PadzConfig = Clapfig::builder()
        .app_name("padz")
        .file_name("padz.toml")
        .search_paths(config_search_paths)
        .search_mode(SearchMode::Merge)
        .strict(false)
        .load()
        .unwrap_or_default();
    // Publish ordering preference to this thread so indexed_pads picks it up.
    crate::index::set_ordering_key(config.ordering);
    let format_ext = config.format_ext();

    // Migrate legacy flat layout to bucketed layout (if needed)
    if let Some(ref project_dir) = project_padz_dir {
        migrate_if_needed(project_dir);
    }
    migrate_if_needed(&global_data_dir);

    let store = FileStore::new_fs(project_padz_dir.clone(), global_data_dir.clone())
        .with_format(&format_ext);
    let paths = PadzPaths {
        project: project_padz_dir,
        global: global_data_dir,
    };
    let api = PadzApi::new(store, paths);

    Ok(PadzContext { api, scope, config })
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

    // --- find_padz_root tests ---

    #[test]
    fn test_find_padz_root_at_cwd() {
        // `.padz` in the current directory is found immediately, no `.git` needed.
        // Regression: previously find_project_root required both and would skip this.
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir(root.join(".padz")).unwrap();

        let result = find_padz_root(root);
        assert_eq!(result, Some(root.to_path_buf()));
    }

    #[test]
    fn test_find_padz_root_walks_up() {
        let temp = TempDir::new().unwrap();
        let parent = temp.path();
        let child = parent.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::create_dir(parent.join(".padz")).unwrap();

        assert_eq!(find_padz_root(&child), Some(parent.to_path_buf()));
    }

    #[test]
    fn test_find_padz_root_inner_wins() {
        // Innermost .padz (closest to cwd) is picked over an outer one.
        let temp = TempDir::new().unwrap();
        let outer = temp.path();
        let inner = outer.join("inner");
        fs::create_dir_all(&inner).unwrap();
        fs::create_dir(outer.join(".padz")).unwrap();
        fs::create_dir(inner.join(".padz")).unwrap();

        assert_eq!(find_padz_root(&inner), Some(inner));
    }

    #[test]
    fn test_find_padz_root_no_match() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("a").join("b");
        fs::create_dir_all(&dir).unwrap();

        assert_eq!(find_padz_root(&dir), None);
    }

    // --- find_git_root tests ---

    #[test]
    fn test_find_git_root_at_cwd() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::create_dir(root.join(".git")).unwrap();

        assert_eq!(find_git_root(root), Some(root.to_path_buf()));
    }

    #[test]
    fn test_find_git_root_walks_up() {
        // Subdir of a git repo resolves to the repo root.
        let temp = TempDir::new().unwrap();
        let repo = temp.path();
        let sub = repo.join("src").join("cli");
        fs::create_dir_all(&sub).unwrap();
        fs::create_dir(repo.join(".git")).unwrap();

        assert_eq!(find_git_root(&sub), Some(repo.to_path_buf()));
    }

    #[test]
    fn test_find_git_root_innermost_wins() {
        // Nested repos: innermost .git is used, not the parent.
        let temp = TempDir::new().unwrap();
        let outer = temp.path();
        let inner = outer.join("submodule");
        fs::create_dir_all(&inner).unwrap();
        fs::create_dir(outer.join(".git")).unwrap();
        fs::create_dir(inner.join(".git")).unwrap();

        assert_eq!(find_git_root(&inner), Some(inner));
    }

    #[test]
    fn test_find_git_root_no_match() {
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("a");
        fs::create_dir_all(&dir).unwrap();

        assert_eq!(find_git_root(&dir), None);
    }

    #[test]
    fn test_find_padz_root_ignores_file_marker() {
        // A regular file named `.padz` must not count as a store. Otherwise an
        // accidental same-named file would mis-detect a project root and blow
        // up when later code tried to create bucket subdirectories under a
        // non-directory path.
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        fs::write(root.join(".padz"), "not a store").unwrap();

        assert_eq!(find_padz_root(root), None);
    }

    #[test]
    fn test_find_git_root_accepts_file_marker() {
        // Git worktrees and submodules use a regular `.git` *file* that points
        // at the real gitdir. `find_git_root` must accept this so auto-init
        // still scopes new pads to the worktree's root.
        let temp = TempDir::new().unwrap();
        let worktree = temp.path().join("worktree");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(
            worktree.join(".git"),
            "gitdir: /elsewhere/.git/worktrees/x\n",
        )
        .unwrap();

        assert_eq!(find_git_root(&worktree), Some(worktree));
    }

    #[test]
    fn test_initialize_write_auto_init_failure_errors() {
        // If auto-init cannot materialize the layout (here: there's already a
        // *file* at the target .padz path so create_dir_all fails), we must
        // error out rather than silently sending the new pad to Global — the
        // user is clearly inside a git repo and expects project scope.
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir(repo.join(".git")).unwrap();
        fs::write(repo.join(".padz"), "blocking file").unwrap();
        let sub = repo.join("src");
        fs::create_dir_all(&sub).unwrap();

        let msg = match initialize(&sub, false, None, true) {
            Ok(_) => panic!("expected auto-init failure, got Ok"),
            Err(e) => e.to_string(),
        };
        assert!(
            msg.contains("could not auto-init"),
            "unexpected error: {msg}"
        );
        assert!(
            msg.contains("padz init"),
            "error should hint at `padz init`: {msg}"
        );
    }

    #[test]
    fn test_initialize_surfaces_broken_link_error() {
        // A `.padz` with a link pointing at a non-existent target used to be
        // swallowed — `initialize` silently fell back to the local (unlinked)
        // directory, which may be a completely different store than the user
        // configured. That is now a hard error on every command so the broken
        // link surfaces instead of misrouting writes.
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(project.join(".padz")).unwrap();
        fs::write(
            project.join(".padz").join("link"),
            "/definitely/not/a/real/path",
        )
        .unwrap();

        let msg = match initialize(&project, false, None, false) {
            Ok(_) => panic!("expected broken-link error, got Ok"),
            Err(e) => e.to_string(),
        };
        assert!(msg.contains("does not exist"), "unexpected error: {msg}");
    }

    #[test]
    fn test_discovery_independent() {
        // The two discovery algorithms are independent: a dir with only `.padz`
        // and no `.git` is found by find_padz_root; a dir with only `.git` and
        // no `.padz` is found by find_git_root. This is the core of the split —
        // the old find_project_root required both at the same location.
        let temp = TempDir::new().unwrap();
        let padz_only = temp.path().join("padz-only");
        let git_only = temp.path().join("git-only");
        fs::create_dir_all(&padz_only).unwrap();
        fs::create_dir_all(&git_only).unwrap();
        fs::create_dir(padz_only.join(".padz")).unwrap();
        fs::create_dir(git_only.join(".git")).unwrap();

        assert_eq!(find_padz_root(&padz_only), Some(padz_only.clone()));
        assert_eq!(find_git_root(&padz_only), None);

        assert_eq!(find_git_root(&git_only), Some(git_only.clone()));
        assert_eq!(find_padz_root(&git_only), None);
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
        let ctx = initialize(repo, false, Some(override_dir.clone()), false).unwrap();

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
        let ctx = initialize(repo, false, Some(override_dir.clone()), false).unwrap();

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
        let ctx = initialize(repo, false, None, false).unwrap();

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
        // Global flag wins: scope is Global, project path is None
        // Note: CLI prevents this combination (--data conflicts with -g)
        let ctx = initialize(repo, true, Some(override_dir), false).unwrap();

        assert_eq!(ctx.api.paths().project, None);
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
        let ctx = initialize(&workdir, false, Some(project.join(".padz")), false).unwrap();

        // Should use project's .padz path
        assert_eq!(ctx.api.paths().project, Some(project.join(".padz")));
    }

    // --- Read-path discovery: .padz alone is enough ---

    #[test]
    fn test_initialize_finds_padz_without_git() {
        // Regression for the bug that motivated the split: a directory with
        // `.padz` but no `.git` used to fall through to global. With the new
        // read-path rules, find_padz_root picks it up.
        let temp = TempDir::new().unwrap();
        let project = temp.path().join("project");
        fs::create_dir_all(project.join(".padz").join("active")).unwrap();
        // No .git on purpose.

        let ctx = initialize(&project, false, None, false).unwrap();

        assert_eq!(ctx.api.paths().project, Some(project.join(".padz")));
        assert_eq!(ctx.scope, Scope::Project);
    }

    // --- Auto-init-on-write discovery ---

    #[test]
    fn test_initialize_auto_inits_at_git_root_on_write() {
        // Write op in a git repo with no `.padz` anywhere should materialize
        // `.padz/` at the git root and return Project scope.
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        let sub = repo.join("src");
        fs::create_dir_all(&sub).unwrap();
        fs::create_dir(repo.join(".git")).unwrap();

        let ctx = initialize(&sub, false, None, true).unwrap();

        assert_eq!(ctx.api.paths().project, Some(repo.join(".padz")));
        assert_eq!(ctx.scope, Scope::Project);
        // Auto-init must have created the bucket layout on disk.
        assert!(repo.join(".padz").join("active").is_dir());
        assert!(repo.join(".padz").join("archived").is_dir());
        assert!(repo.join(".padz").join("deleted").is_dir());
    }

    #[test]
    fn test_initialize_read_never_auto_inits() {
        // Same setup as the auto-init test, but `auto_init_for_write = false`.
        // Should fall back to Global and leave the filesystem untouched.
        let temp = TempDir::new().unwrap();
        let repo = temp.path().join("repo");
        let sub = repo.join("src");
        fs::create_dir_all(&sub).unwrap();
        fs::create_dir(repo.join(".git")).unwrap();

        let ctx = initialize(&sub, false, None, false).unwrap();

        assert_eq!(ctx.scope, Scope::Global);
        assert_eq!(ctx.api.paths().project, None);
        assert!(!repo.join(".padz").exists());
    }

    #[test]
    fn test_initialize_write_outside_git_goes_global() {
        // No `.padz`, no `.git` → global, even on a write op.
        let temp = TempDir::new().unwrap();
        let dir = temp.path().join("loose").join("dir");
        fs::create_dir_all(&dir).unwrap();

        let ctx = initialize(&dir, false, None, true).unwrap();

        assert_eq!(ctx.scope, Scope::Global);
        assert_eq!(ctx.api.paths().project, None);
    }

    #[test]
    fn test_initialize_write_prefers_existing_padz_over_git() {
        // Parent has `.padz`, child has `.git` with no `.padz`. The write should
        // use the parent's existing store, not auto-init a new one at the child.
        // (The read discovery matches first and short-circuits.)
        let temp = TempDir::new().unwrap();
        let parent = temp.path().join("parent");
        let child = parent.join("child");
        fs::create_dir_all(&child).unwrap();
        fs::create_dir(parent.join(".padz")).unwrap();
        fs::create_dir(child.join(".git")).unwrap();

        let ctx = initialize(&child, false, None, true).unwrap();

        assert_eq!(ctx.api.paths().project, Some(parent.join(".padz")));
        // Child must not have had a .padz created under it.
        assert!(!child.join(".padz").exists());
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
        let ctx = initialize(&project_b, false, None, false).unwrap();
        assert_eq!(
            ctx.api.paths().project,
            Some(project_a.canonicalize().unwrap().join(".padz"))
        );
    }
}
