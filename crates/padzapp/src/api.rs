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

    pub fn get_pads(&self, scope: Scope, filter: PadFilter) -> Result<commands::CmdResult> {
        commands::get::run(&self.store, scope, filter)
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

    pub fn update_pads(
        &mut self,
        scope: Scope,
        updates: &[commands::PadUpdate],
    ) -> Result<commands::CmdResult> {
        commands::update::run(&mut self.store, scope, updates)
    }

    pub fn purge_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        skip_confirm: bool,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::purge::run(&mut self.store, scope, &selectors, skip_confirm)
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

    pub fn config(&self, scope: Scope, action: ConfigAction) -> Result<commands::CmdResult> {
        commands::config::run(&self.paths, scope, action)
    }

    pub fn init(&self, scope: Scope) -> Result<commands::CmdResult> {
        commands::init::run(&self.paths, scope)
    }

    pub fn paths(&self) -> &commands::PadzPaths {
        &self.paths
    }
}

fn parse_selectors<I: AsRef<str>>(inputs: &[I]) -> Result<Vec<PadSelector>> {
    // 1. Try to parse ALL inputs as DisplayIndex (including ranges like "3-5")
    let mut all_selectors = Vec::new();
    let mut parse_failed = false;

    for input in inputs {
        match parse_index_or_range(input.as_ref()) {
            Ok(selector) => all_selectors.push(selector),
            Err(_e) => {
                // Check if it's a specific error (though index.rs now only parses syntax)
                // If it fails syntax, we assume it's a title search
                parse_failed = true;
                break;
            }
        }
    }

    if !parse_failed {
        // Deduplicate? Paths might be dupe.
        // We can't easily dedup PadSelector without Hash/Eq, which it should derive.
        // But PadSelector::Range/Path contains Vec which are Hash/Eq.
        // But for now let's just return all of them.
        // To be safe against dupes, we can convert to string and dedup?
        // Or implement Hash for PadSelector in index.rs (it derived PartialEq, Eq).
        // Let's assume index.rs added Hash.

        // Wait, PadSelector needs Hash for HashSet.
        // I'll add Hash to PadSelector derive in index.rs?
        // It has Vec<DisplayIndex>. DisplayIndex is Hash.
        // So yes, I should add Hash to PadSelector.

        let mut unique_selectors = Vec::new();
        // Simple dedup
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

pub use crate::commands::config::ConfigAction;
pub use commands::get::{PadFilter, PadStatusFilter};
pub use commands::{CmdMessage, CmdResult, MessageLevel, PadUpdate, PadzPaths};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{DisplayIndex, PadSelector};
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
}
