//! # API Facade
//!
//! The API layer is a **thin facade** over the command layer. It serves as the single
//! entry point for all padz operations, regardless of the UI being used.
//!
//! ## Role and Responsibilities
//!
//! The API facade:
//! - **Dispatches** to the appropriate command function
//! - **Normalizes inputs** (e.g., converting display indexes to UUIDs)
//! - **Returns structured types** (`Result<CmdResult>`)
//!
//! ## What the API Does NOT Do
//!
//! The API explicitly avoids:
//! - **Business logic**: That belongs in `commands/*.rs`
//! - **I/O operations**: No stdout, stderr, or file formatting
//! - **Presentation concerns**: Returns data structures, not strings
//!
//! ## Consistent Output Representation
//!
//! All pad data in [`CmdResult`] uses [`DisplayPad`], which pairs a [`Pad`] with its
//! canonical [`DisplayIndex`]. This applies to both:
//! - `affected_pads`: Pads modified by the operation (with post-operation index)
//! - `listed_pads`: Pads returned for display (with current index)
//!
//! Clients receive a uniform representation regardless of the operation type.
//!
//! ## Selectors: Multi-IDs and Ranges
//!
//! Users often need to act on batches of items (`padz delete 1-3`).
//! The API handles parsing and resolution of these selectors.
//!
//! ### Selector Grammar
//!
//! - **Regular Index**: `N` (e.g., `1`, `42`)
//! - **Pinned Index**: `pX` (e.g., `p1`, `p2`)
//! - **Deleted Index**: `dX` (e.g., `d1`, `d5`)
//! - **Ranges**: `Start-End` (e.g., `1-5`, `p1-p3`)
//!   - Must be homogeneous: `1-p3` is **invalid**
//!   - Start must be ≤ end: `5-3` is **invalid**
//!
//! ### Processing Pipeline
//!
//! 1. **Parsing**: [`crate::index::parse_index_or_range`] expands `"1-3"` → `[Regular(1), Regular(2), Regular(3)]`
//! 2. **Resolution**: `parse_selectors` converts to `Vec<PadSelector>`, deduplicating while preserving order
//! 3. **Search Fallback**: If parsing fails, the entire input becomes a title search term
//!
//! ### Special: Restore and Purge
//!
//! For commands on deleted pads, bare numbers auto-prefix with `d`:
//! - `padz restore 3` → Internally becomes `d3`
//! - `padz restore 1-3` → Internally becomes `d1-d3`
//!
//! See `parse_selectors_for_deleted` for implementation.
//!
//! ## Generic Over DataStore
//!
//! `PadzApi<S: DataStore>` is generic over the storage backend:
//! - Production: `PadzApi<FileStore>`
//! - Testing: `PadzApi<InMemoryStore>`
//!
//! This enables testing the API layer without touching the filesystem.
//!
//! ## Testing Strategy
//!
//! API tests should verify:
//! - Correct command is called for each method
//! - Arguments are passed/transformed correctly
//! - Return types are appropriate
//!
//! API tests should **not** verify:
//! - Command logic (tested in command modules)
//! - Storage behavior (tested in store modules)

use crate::commands;
use crate::error::{PadzError, Result};
use crate::index::{parse_index_or_range, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;

/// The main API facade for padz operations.
///
/// Generic over `DataStore` to allow different storage backends.
/// All UI clients (CLI, web, etc.) should interact through this API.
pub struct PadzApi<S: DataStore> {
    store: S,
    paths: commands::PadzPaths,
}

impl<S: DataStore> PadzApi<S> {
    pub fn new(store: S, paths: commands::PadzPaths) -> Self {
        Self { store, paths }
    }

    pub fn create_pad(
        &mut self,
        scope: Scope,
        title: String,
        content: String,
        parent: Option<&str>,
    ) -> Result<commands::CmdResult> {
        let parent_selector = if let Some(p) = parent {
            Some(parse_index_or_range(p).map_err(PadzError::Api)?)
        } else {
            None
        };
        commands::create::run(&mut self.store, scope, title, content, parent_selector)
    }

    pub fn get_pads<I: AsRef<str>>(
        &self,
        scope: Scope,
        filter: PadFilter,
        ids: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = if ids.is_empty() {
            vec![]
        } else {
            parse_selectors(ids)?
        };
        commands::get::run(&self.store, scope, filter, &selectors)
    }

    pub fn view_pads<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::view::run(&self.store, scope, &selectors)
    }

    pub fn delete_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::delete::run(&mut self.store, scope, &selectors)
    }

    pub fn pin_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::pinning::pin(&mut self.store, scope, &selectors)
    }

    pub fn unpin_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::pinning::unpin(&mut self.store, scope, &selectors)
    }

    /// Marks pads as Done (completed).
    pub fn complete_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::status::complete(&mut self.store, scope, &selectors)
    }

    /// Reopens pads (sets them back to Planned).
    pub fn reopen_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::status::reopen(&mut self.store, scope, &selectors)
    }

    pub fn update_pads(
        &mut self,
        scope: Scope,
        updates: &[commands::PadUpdate],
    ) -> Result<commands::CmdResult> {
        commands::update::run(&mut self.store, scope, updates)
    }

    /// Updates pads with raw content (e.g., from piped stdin).
    ///
    /// Parses the raw content to extract title and body, then updates
    /// the pads matching the given selectors.
    pub fn update_pads_from_content<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        raw_content: &str,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::update::run_from_content(&mut self.store, scope, &selectors, raw_content)
    }

    pub fn move_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        to_parent: Option<&str>,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        let parent_selector = if let Some(p) = to_parent {
            if p.trim().is_empty() {
                None // Empty string means move to root
            } else {
                Some(parse_index_or_range(p).map_err(PadzError::Api)?)
            }
        } else {
            None
        };
        commands::move_pads::run(&mut self.store, scope, &selectors, parent_selector.as_ref())
    }

    /// Permanently deletes pads.
    ///
    /// **Confirmation required**: The `confirmed` parameter must be `true` to proceed.
    /// If `false`, returns an error with a message instructing to use `--yes` or `-y`.
    pub fn purge_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        recursive: bool,
        confirmed: bool,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::purge::run(&mut self.store, scope, &selectors, recursive, confirmed)
    }

    pub fn restore_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        // Normalize inputs: bare numbers become deleted indexes (e.g., "3" -> "d3")
        let selectors = parse_selectors_for_deleted(indexes)?;
        commands::restore::run(&mut self.store, scope, &selectors)
    }

    pub fn export_pads<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::export::run(&self.store, scope, &selectors)
    }

    pub fn export_pads_single_file<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
        title: &str,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::export::run_single_file(&self.store, scope, &selectors, title)
    }

    pub fn import_pads(
        &mut self,
        scope: Scope,
        paths: Vec<std::path::PathBuf>,
        import_exts: &[String],
    ) -> Result<commands::CmdResult> {
        commands::import::run(&mut self.store, scope, paths, import_exts)
    }

    pub fn doctor(&mut self, scope: Scope) -> Result<commands::CmdResult> {
        commands::doctor::run(&mut self.store, scope)
    }

    pub fn pad_paths<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::paths::run(&self.store, scope, &selectors)
    }

    pub fn get_path_by_id(&self, scope: Scope, id: uuid::Uuid) -> Result<std::path::PathBuf> {
        self.store.get_pad_path(&id, scope)
    }

    pub fn init(&self, scope: Scope) -> Result<commands::CmdResult> {
        commands::init::run(&self.paths, scope)
    }

    pub fn paths(&self) -> &commands::PadzPaths {
        &self.paths
    }

    // --- Tag Management ---

    /// List all tags in the registry.
    pub fn list_tags(&self, scope: Scope) -> Result<commands::CmdResult> {
        commands::tags::list_tags(&self.store, scope)
    }

    /// Create a new tag in the registry.
    pub fn create_tag(&mut self, scope: Scope, name: &str) -> Result<commands::CmdResult> {
        commands::tags::create_tag(&mut self.store, scope, name)
    }

    /// Delete a tag from the registry (cascades to remove from all pads).
    pub fn delete_tag(&mut self, scope: Scope, name: &str) -> Result<commands::CmdResult> {
        commands::tags::delete_tag(&mut self.store, scope, name)
    }

    /// Rename a tag in the registry (updates all pads).
    pub fn rename_tag(
        &mut self,
        scope: Scope,
        old_name: &str,
        new_name: &str,
    ) -> Result<commands::CmdResult> {
        commands::tags::rename_tag(&mut self.store, scope, old_name, new_name)
    }

    // --- Pad Tagging ---

    /// Add tags to pads.
    pub fn add_tags_to_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        tags: &[String],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::tagging::add_tags(&mut self.store, scope, &selectors, tags)
    }

    /// Remove tags from pads.
    pub fn remove_tags_from_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        tags: &[String],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::tagging::remove_tags(&mut self.store, scope, &selectors, tags)
    }

    /// Clear all tags from pads.
    pub fn clear_tags_from_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::tagging::clear_tags(&mut self.store, scope, &selectors)
    }
}

fn parse_selectors<I: AsRef<str>>(inputs: &[I]) -> Result<Vec<PadSelector>> {
    // 1. Try to parse ALL inputs as DisplayIndex (including ranges like "3-5")
    let mut all_selectors = Vec::new();
    let mut parse_failed = false;

    for input in inputs {
        match parse_index_or_range(input.as_ref()) {
            Ok(selector) => all_selectors.push(selector),
            Err(_) => {
                // Non-index input - treat entire input as title search
                parse_failed = true;
                break;
            }
        }
    }

    if !parse_failed {
        // Deduplicate while preserving order
        let mut unique_selectors = Vec::new();
        for s in all_selectors {
            if !unique_selectors.contains(&s) {
                unique_selectors.push(s);
            }
        }
        return Ok(unique_selectors);
    }

    // 2. If any failed (meaning there are non-index strings), treat as ONE search query
    // Join all parts with space
    let search_term = inputs
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<&str>>()
        .join(" ");

    Ok(vec![PadSelector::Title(search_term)])
}

/// Parses selectors for commands that operate on deleted pads (restore, purge).
/// Bare numbers are treated as deleted indexes: "3" -> "d3", but "d3" stays "d3".
fn parse_selectors_for_deleted<I: AsRef<str>>(inputs: &[I]) -> Result<Vec<PadSelector>> {
    let normalized: Vec<String> = inputs
        .iter()
        .map(|s| normalize_to_deleted_index(s.as_ref()))
        .collect();

    parse_selectors(&normalized)
}

/// Normalizes an index string to a deleted index if it's a bare number.
/// "3" -> "d3", "d3" -> "d3", "p1" -> "p1", "3-5" -> "d3-d5"
fn normalize_to_deleted_index(s: &str) -> String {
    if let Some(dash_pos) = s.find('-') {
        if dash_pos > 0 {
            let start_str = &s[..dash_pos];
            let end_str = &s[dash_pos + 1..];
            let normalized_start = normalize_path_for_deleted(start_str);
            let normalized_end = normalize_path_for_deleted(end_str);
            return format!("{}-{}", normalized_start, normalized_end);
        }
    }
    normalize_path_for_deleted(s)
}

/// Normalizes a path string (e.g., "1", "1.2", "p1") for deleted operations.
/// This means the *last* segment of a path, if it's a bare number, gets prefixed with 'd'.
/// "3" -> "d3"
/// "1.2" -> "1.d2"
/// "p1.2" -> "p1.d2"
/// "d1.2" -> "d1.d2"
fn normalize_path_for_deleted(s: &str) -> String {
    let mut parts: Vec<String> = s.split('.').map(|s| s.to_string()).collect();
    if let Some(last) = parts.last_mut() {
        *last = normalize_single_to_deleted(last);
    }
    parts.join(".")
}

/// Normalizes a single index (not a range) to deleted format.
/// "3" -> "d3", "d3" -> "d3", "p1" -> "p1"
fn normalize_single_to_deleted(s: &str) -> String {
    // If it's a bare number, prefix with 'd'
    if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
        format!("d{}", s)
    } else {
        s.to_string()
    }
}

// parse_index_or_range and parse_path removed (imported from index.rs)

pub use crate::model::TodoStatus;
pub use commands::get::{PadFilter, PadStatusFilter};
pub use commands::{CmdMessage, CmdResult, MessageLevel, PadUpdate, PadzPaths};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{DisplayIndex, PadSelector};
    use crate::store::memory::InMemoryStore;
    use std::path::PathBuf;

    fn make_api() -> PadzApi<InMemoryStore> {
        let store = InMemoryStore::new();
        let paths = PadzPaths {
            project: Some(PathBuf::from("/tmp/test")),
            global: PathBuf::from("/tmp/global"),
        };
        PadzApi::new(store, paths)
    }

    // --- parse_selectors tests ---

    #[test]
    fn test_parse_selectors_single_index() {
        let inputs = vec!["1"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        assert!(matches!(selectors[0], PadSelector::Path(_)));
    }

    #[test]
    fn test_parse_selectors_multiple_indexes() {
        let inputs = vec!["1", "3", "5"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 3);
    }

    #[test]
    fn test_parse_selectors_deduplicates() {
        let inputs = vec!["1", "1", "2", "1"];
        let selectors = parse_selectors(&inputs).unwrap();

        // Should deduplicate: [1, 2]
        assert_eq!(selectors.len(), 2);
    }

    #[test]
    fn test_parse_selectors_title_fallback() {
        let inputs = vec!["meeting", "notes"];
        let selectors = parse_selectors(&inputs).unwrap();

        // Should become title search
        assert_eq!(selectors.len(), 1);
        match &selectors[0] {
            PadSelector::Title(term) => assert_eq!(term, "meeting notes"),
            _ => panic!("Expected Title selector"),
        }
    }

    #[test]
    fn test_parse_selectors_mixed_input_becomes_title() {
        let inputs = vec!["1", "meeting", "2"];
        let selectors = parse_selectors(&inputs).unwrap();

        // Mixed input becomes title search
        assert_eq!(selectors.len(), 1);
        match &selectors[0] {
            PadSelector::Title(term) => assert_eq!(term, "1 meeting 2"),
            _ => panic!("Expected Title selector"),
        }
    }

    #[test]
    fn test_parse_selectors_range() {
        let inputs = vec!["1-3"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        assert!(matches!(selectors[0], PadSelector::Range(_, _)));
    }

    // --- API facade integration tests ---

    #[test]
    fn test_api_create_pad_simple() {
        let mut api = make_api();

        let result = api
            .create_pad(Scope::Project, "Test Title".into(), "Content".into(), None)
            .unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "Test Title");
    }

    #[test]
    fn test_api_create_pad_with_parent_string() {
        let mut api = make_api();

        // Create parent first
        api.create_pad(Scope::Project, "Parent".into(), "".into(), None)
            .unwrap();

        // Create child with parent string
        let result = api
            .create_pad(Scope::Project, "Child".into(), "".into(), Some("1"))
            .unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "Child");
        assert!(result.affected_pads[0].pad.metadata.parent_id.is_some());
    }

    #[test]
    fn test_api_get_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();

        let result = api
            .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
            .unwrap();

        assert_eq!(result.listed_pads.len(), 1);
    }

    #[test]
    fn test_api_view_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();

        let result = api.view_pads(Scope::Project, &["1"]).unwrap();

        assert_eq!(result.listed_pads.len(), 1);
        assert_eq!(result.listed_pads[0].pad.metadata.title, "Test");
    }

    #[test]
    fn test_api_delete_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();

        let result = api.delete_pads(Scope::Project, &["1"]).unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert!(result.affected_pads[0].pad.metadata.is_deleted);
    }

    #[test]
    fn test_api_pin_unpin_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();

        // Pin
        let result = api.pin_pads(Scope::Project, &["1"]).unwrap();
        assert_eq!(result.affected_pads.len(), 1);
        assert!(result.affected_pads[0].pad.metadata.is_pinned);

        // Unpin
        let result = api.unpin_pads(Scope::Project, &["p1"]).unwrap();
        assert_eq!(result.affected_pads.len(), 1);
        assert!(!result.affected_pads[0].pad.metadata.is_pinned);
    }

    #[test]
    fn test_api_update_pads() {
        let mut api = make_api();
        api.create_pad(
            Scope::Project,
            "Old Title".into(),
            "Old Content".into(),
            None,
        )
        .unwrap();

        let updates = vec![PadUpdate::new(
            DisplayIndex::Regular(1),
            "New Title".into(),
            "New Content".into(),
        )];
        let result = api.update_pads(Scope::Project, &updates).unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "New Title");
    }

    #[test]
    fn test_api_restore_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.delete_pads(Scope::Project, &["1"]).unwrap();

        // Restore using bare number (should be normalized to d1)
        let result = api.restore_pads(Scope::Project, &["1"]).unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert!(!result.affected_pads[0].pad.metadata.is_deleted);
    }

    #[test]
    fn test_api_purge_pads_confirmed() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.delete_pads(Scope::Project, &["1"]).unwrap();

        // Purge with confirmed=true should succeed
        let result = api.purge_pads(Scope::Project, &["d1"], false, true);
        assert!(result.is_ok());

        // Verify it's gone from the store
        let list = api
            .get_pads(
                Scope::Project,
                PadFilter {
                    status: PadStatusFilter::All,
                    search_term: None,
                    todo_status: None,
                    tags: None,
                },
                &[] as &[String],
            )
            .unwrap();
        assert_eq!(list.listed_pads.len(), 0);
    }

    #[test]
    fn test_api_purge_pads_not_confirmed() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.delete_pads(Scope::Project, &["1"]).unwrap();

        // Purge with confirmed=false should fail
        let result = api.purge_pads(Scope::Project, &["d1"], false, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Aborted"));

        // Verify pad is still there
        let list = api
            .get_pads(
                Scope::Project,
                PadFilter {
                    status: PadStatusFilter::Deleted,
                    search_term: None,
                    todo_status: None,
                    tags: None,
                },
                &[] as &[String],
            )
            .unwrap();
        assert_eq!(list.listed_pads.len(), 1);
    }

    #[test]
    fn test_api_doctor() {
        let mut api = make_api();

        let result = api.doctor(Scope::Project).unwrap();

        // Should return success message for clean store
        assert!(!result.messages.is_empty());
    }

    #[test]
    fn test_api_paths_accessor() {
        let api = make_api();

        let paths = api.paths();

        assert_eq!(paths.project, Some(PathBuf::from("/tmp/test")));
        assert_eq!(paths.global, PathBuf::from("/tmp/global"));
    }

    #[test]
    fn test_api_import_pads() {
        let mut api = make_api();
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("note.md");
        std::fs::write(&file_path, "Imported Note\n\nContent").unwrap();

        let result = api
            .import_pads(Scope::Project, vec![file_path], &[".md".to_string()])
            .unwrap();

        assert!(result
            .messages
            .iter()
            .any(|m| m.content.contains("Total imported: 1")));
    }

    #[test]
    fn test_normalize_single_to_deleted() {
        // Bare numbers get 'd' prefix
        assert_eq!(normalize_single_to_deleted("1"), "d1");
        assert_eq!(normalize_single_to_deleted("42"), "d42");

        // Already prefixed stays the same
        assert_eq!(normalize_single_to_deleted("d1"), "d1");
        assert_eq!(normalize_single_to_deleted("d42"), "d42");

        // Pinned indexes stay as-is
        assert_eq!(normalize_single_to_deleted("p1"), "p1");
        assert_eq!(normalize_single_to_deleted("p99"), "p99");

        // Empty string stays empty
        assert_eq!(normalize_single_to_deleted(""), "");

        // Non-numeric stays the same
        assert_eq!(normalize_single_to_deleted("abc"), "abc");
    }

    #[test]
    fn test_normalize_to_deleted_index_ranges() {
        // Bare number ranges get 'd' prefix on both sides
        assert_eq!(normalize_to_deleted_index("3-5"), "d3-d5");
        assert_eq!(normalize_to_deleted_index("1-10"), "d1-d10");

        // Already deleted ranges stay the same
        assert_eq!(normalize_to_deleted_index("d3-d5"), "d3-d5");

        // Mixed input: bare start, prefixed end
        assert_eq!(normalize_to_deleted_index("3-d5"), "d3-d5");

        // Mixed input: prefixed start, bare end
        assert_eq!(normalize_to_deleted_index("d3-5"), "d3-d5");

        // Single indexes (no range)
        assert_eq!(normalize_to_deleted_index("3"), "d3");
        assert_eq!(normalize_to_deleted_index("d3"), "d3");

        // Hierarchical paths
        assert_eq!(normalize_to_deleted_index("1.2"), "1.d2");
        assert_eq!(normalize_to_deleted_index("p1.2"), "p1.d2");
        assert_eq!(normalize_to_deleted_index("d1.2"), "d1.d2");
        assert_eq!(normalize_to_deleted_index("1.p2"), "1.p2");
        assert_eq!(normalize_to_deleted_index("1.d2"), "1.d2");
        assert_eq!(normalize_to_deleted_index("1.2-1.4"), "1.d2-1.d4");
        assert_eq!(normalize_to_deleted_index("d1.2-d1.4"), "d1.d2-d1.d4");
    }

    #[test]
    fn test_parse_selectors_for_deleted() {
        // Test that bare numbers become deleted indexes
        let inputs = vec!["1", "3", "d5"];
        let selectors = parse_selectors_for_deleted(&inputs).unwrap();

        assert_eq!(selectors.len(), 3);
        // They are Paths now
        assert!(matches!(selectors[0], PadSelector::Path(_)));
        if let PadSelector::Path(path) = &selectors[0] {
            assert_eq!(path.len(), 1);
            assert!(matches!(path[0], DisplayIndex::Deleted(1)));
        }
        if let PadSelector::Path(path) = &selectors[1] {
            assert_eq!(path.len(), 1);
            assert!(matches!(path[0], DisplayIndex::Deleted(3)));
        }
        if let PadSelector::Path(path) = &selectors[2] {
            assert_eq!(path.len(), 1);
            assert!(matches!(path[0], DisplayIndex::Deleted(5)));
        }
    }

    #[test]
    fn test_parse_selectors_for_deleted_with_range() {
        // Test range normalization
        let inputs = vec!["1-3"];
        let selectors = parse_selectors_for_deleted(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        match &selectors[0] {
            PadSelector::Range(start, end) => {
                assert_eq!(start.len(), 1);
                assert!(matches!(start[0], DisplayIndex::Deleted(1)));
                assert_eq!(end.len(), 1);
                assert!(matches!(end[0], DisplayIndex::Deleted(3)));
            }
            _ => panic!("Expected Range"),
        }
    }

    #[test]
    fn test_parse_selectors_for_deleted_with_hierarchical_range() {
        let inputs = vec!["1.2-1.4"];
        let selectors = parse_selectors_for_deleted(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        match &selectors[0] {
            PadSelector::Range(start_path, end_path) => {
                assert_eq!(start_path.len(), 2);
                assert!(matches!(start_path[0], DisplayIndex::Regular(1)));
                assert!(matches!(start_path[1], DisplayIndex::Deleted(2)));

                assert_eq!(end_path.len(), 2);
                assert!(matches!(end_path[0], DisplayIndex::Regular(1)));
                assert!(matches!(end_path[1], DisplayIndex::Deleted(4)));
            }
            _ => panic!("Expected PadSelector::Range"),
        }
    }

    #[test]
    fn test_api_move_pads() {
        let mut api = make_api();
        // Create A (2 - Oldest)
        api.create_pad(Scope::Project, "A".into(), "".into(), None)
            .unwrap();

        // Ensure timestamp difference
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Create B (1 - Newest)
        api.create_pad(Scope::Project, "B".into(), "".into(), None)
            .unwrap();

        // Move B (1) to A (2)
        let result = api.move_pads(Scope::Project, &["1"], Some("2")).unwrap();

        // No messages - CLI handles unified rendering
        assert!(result.messages.is_empty());
        // Should have 1 affected pad
        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "B");

        // Verify hierarchy
        let pads = api
            .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
            .unwrap()
            .listed_pads;
        let pad_a = pads.iter().find(|p| p.pad.metadata.title == "A").unwrap();
        assert_eq!(pad_a.children.len(), 1);
        assert_eq!(pad_a.children[0].pad.metadata.title, "B");
    }

    #[test]
    fn test_api_move_pads_to_root() {
        let mut api = make_api();
        // Create Parent
        api.create_pad(Scope::Project, "Parent".into(), "".into(), None)
            .unwrap();
        // Create Child
        api.create_pad(Scope::Project, "Child".into(), "".into(), Some("1"))
            .unwrap();

        // Move Child (1.1) to Root
        let result = api.move_pads(Scope::Project, &["1.1"], None).unwrap();
        // No messages - CLI handles unified rendering
        assert!(result.messages.is_empty());
        // Should have 1 affected pad
        assert_eq!(result.affected_pads.len(), 1);

        // Verify Child is now root
        let pads = api
            .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
            .unwrap()
            .listed_pads;
        let child = pads
            .iter()
            .find(|p| p.pad.metadata.title == "Child")
            .unwrap();
        assert!(child.pad.metadata.parent_id.is_none());
    }

    #[test]
    fn test_api_move_pads_cycle_error() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "A".into(), "".into(), None)
            .unwrap();

        // Move 1 to 1 should fail
        let result = api.move_pads(Scope::Project, &["1"], Some("1"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("into itself"));
    }

    // --- Tag Management API tests ---

    #[test]
    fn test_api_list_tags() {
        let mut api = make_api();
        api.create_tag(Scope::Project, "work").unwrap();

        let result = api.list_tags(Scope::Project).unwrap();

        assert!(result.messages.iter().any(|m| m.content.contains("work")));
    }

    #[test]
    fn test_api_create_tag() {
        let mut api = make_api();

        let result = api.create_tag(Scope::Project, "rust").unwrap();

        assert!(result.messages[0].content.contains("Created tag 'rust'"));
    }

    #[test]
    fn test_api_delete_tag() {
        let mut api = make_api();
        api.create_tag(Scope::Project, "work").unwrap();

        let result = api.delete_tag(Scope::Project, "work").unwrap();

        assert!(result.messages[0].content.contains("Deleted tag 'work'"));
    }

    #[test]
    fn test_api_rename_tag() {
        let mut api = make_api();
        api.create_tag(Scope::Project, "old-name").unwrap();

        let result = api
            .rename_tag(Scope::Project, "old-name", "new-name")
            .unwrap();

        assert!(result.messages[0]
            .content
            .contains("Renamed tag 'old-name' to 'new-name'"));
    }

    // --- Pad Tagging API tests ---

    #[test]
    fn test_api_add_tags_to_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.create_tag(Scope::Project, "work").unwrap();

        let result = api
            .add_tags_to_pads(Scope::Project, &["1"], &["work".to_string()])
            .unwrap();

        assert!(result.messages[0].content.contains("Added tag"));
        assert!(result.affected_pads[0]
            .pad
            .metadata
            .tags
            .contains(&"work".to_string()));
    }

    #[test]
    fn test_api_remove_tags_from_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.create_tag(Scope::Project, "work").unwrap();
        api.add_tags_to_pads(Scope::Project, &["1"], &["work".to_string()])
            .unwrap();

        let result = api
            .remove_tags_from_pads(Scope::Project, &["1"], &["work".to_string()])
            .unwrap();

        assert!(result.messages[0].content.contains("Removed tag"));
        assert!(result.affected_pads[0].pad.metadata.tags.is_empty());
    }

    #[test]
    fn test_api_clear_tags_from_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.create_tag(Scope::Project, "work").unwrap();
        api.create_tag(Scope::Project, "rust").unwrap();
        api.add_tags_to_pads(
            Scope::Project,
            &["1"],
            &["work".to_string(), "rust".to_string()],
        )
        .unwrap();

        let result = api.clear_tags_from_pads(Scope::Project, &["1"]).unwrap();

        assert!(result.messages[0].content.contains("Cleared tags"));
        assert!(result.affected_pads[0].pad.metadata.tags.is_empty());
    }
}
