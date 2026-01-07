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
use crate::index::{parse_index_or_range, DisplayIndex, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;
use std::collections::HashSet;

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
    ) -> Result<commands::CmdResult> {
        commands::create::run(&mut self.store, scope, title, content)
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
    let mut all_indexes: Vec<DisplayIndex> = Vec::new();
    let mut parse_failed = false;

    for input in inputs {
        match parse_index_or_range(input.as_ref()) {
            Ok(indexes) => all_indexes.extend(indexes),
            Err(e) => {
                // Check if it's a range error (explicit error message) vs just not an index
                if e.contains("Invalid range") || e.contains("cannot mix") {
                    return Err(PadzError::Api(e));
                }
                parse_failed = true;
                break;
            }
        }
    }

    if !parse_failed {
        // Deduplicate while preserving order
        let mut seen = HashSet::new();
        let unique_indexes: Vec<DisplayIndex> = all_indexes
            .into_iter()
            .filter(|idx| seen.insert(idx.clone()))
            .collect();

        return Ok(unique_indexes.into_iter().map(PadSelector::Index).collect());
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
    // Handle ranges: if it contains a dash (not at start), normalize both parts
    if let Some(dash_pos) = s.find('-') {
        if dash_pos > 0 {
            let start = &s[..dash_pos];
            let end = &s[dash_pos + 1..];
            let norm_start = normalize_single_to_deleted(start);
            let norm_end = normalize_single_to_deleted(end);
            return format!("{}-{}", norm_start, norm_end);
        }
    }
    normalize_single_to_deleted(s)
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

pub use crate::commands::config::ConfigAction;
pub use commands::get::{PadFilter, PadStatusFilter};
pub use commands::{CmdMessage, CmdResult, MessageLevel, PadUpdate, PadzPaths};

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn test_parse_selectors_for_deleted() {
        // Test that bare numbers become deleted indexes
        let inputs = vec!["1", "3", "d5"];
        let selectors = parse_selectors_for_deleted(&inputs).unwrap();

        assert_eq!(selectors.len(), 3);
        assert_eq!(selectors[0], PadSelector::Index(DisplayIndex::Deleted(1)));
        assert_eq!(selectors[1], PadSelector::Index(DisplayIndex::Deleted(3)));
        assert_eq!(selectors[2], PadSelector::Index(DisplayIndex::Deleted(5)));
    }

    #[test]
    fn test_parse_selectors_for_deleted_with_range() {
        // Test range normalization
        let inputs = vec!["1-3"];
        let selectors = parse_selectors_for_deleted(&inputs).unwrap();

        assert_eq!(selectors.len(), 3);
        assert_eq!(selectors[0], PadSelector::Index(DisplayIndex::Deleted(1)));
        assert_eq!(selectors[1], PadSelector::Index(DisplayIndex::Deleted(2)));
        assert_eq!(selectors[2], PadSelector::Index(DisplayIndex::Deleted(3)));
    }
}
