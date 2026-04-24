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
//! ## Module layout
//!
//! The public surface (`PadzApi<S>`, its methods, and the re-exports below) is
//! unchanged from before the split. Methods are grouped by domain:
//!
//! - [`crud`] — create / get / view / delete / update / restore / purge / archive
//! - [`status`] — pin / unpin / complete / reopen / move / propagate
//! - [`transfer`] — export / import / clone / migrate
//! - [`tags`] — tag registry CRUD + per-pad tagging
//! - [`init`] — store initialization and linking
//! - [`util`] — paths, uuids, refresh, remove, doctor
//! - [`format`] — `FileStore`-specific create-with-format override
//! - [`selectors`] — internal input-normalization (private)
//!
//! ## Selectors: Multi-IDs and Ranges
//!
//! Users often need to act on batches of items (`padz delete 1-3`).
//! See [`selectors`] for the parsing/normalization layer.

use crate::commands;
use crate::store::DataStore;

mod crud;
mod format;
mod init;
mod selectors;
mod status;
mod tags;
mod transfer;
mod util;

#[cfg(test)]
pub(crate) mod test_support;

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

    pub fn paths(&self) -> &commands::PadzPaths {
        &self.paths
    }
}

pub use crate::model::TodoStatus;
pub use commands::get::{PadFilter, PadStatusFilter};
pub use commands::{CmdMessage, CmdResult, MessageLevel, PadUpdate, PadzPaths};

#[cfg(test)]
mod tests {
    use super::test_support::make_api;
    use super::PadFilter;
    use crate::model::Scope;
    use crate::store::backend::StorageBackend;

    /// Regression test: simulates the full editor flow for nested pad creation.
    ///
    /// The sequence is: create empty pad with parent → (editor fills content)
    /// → refresh_pad → propagate_status. Before the fix, propagation was called
    /// inside create, which triggered reconciliation that deleted the empty file
    /// and its index entry (with parent_id). The pad was then recovered as an
    /// orphan with parent_id: None.
    #[test]
    fn test_editor_flow_preserves_nested_parent() {
        let mut api = make_api();

        api.create_pad(Scope::Project, "Parent".into(), "".into(), None)
            .unwrap();

        let result = api
            .create_pad(Scope::Project, "".into(), "".into(), Some("1"))
            .unwrap();
        let child_id = result.affected_pads[0].pad.metadata.id;
        assert!(result.affected_pads[0].pad.metadata.parent_id.is_some());

        api.store
            .active_store_mut()
            .backend
            .write_content(&child_id, Scope::Project, "Editor Content")
            .unwrap();

        let refreshed = api.refresh_pad(Scope::Project, &child_id).unwrap();
        assert!(refreshed.is_some(), "Pad should exist after refresh");
        let pad = refreshed.unwrap();
        assert_eq!(pad.metadata.title, "Editor Content");
        assert!(
            pad.metadata.parent_id.is_some(),
            "Parent ID must be preserved through editor flow"
        );

        api.propagate_status(Scope::Project, pad.metadata.parent_id)
            .unwrap();

        let all = api
            .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
            .unwrap();
        assert_eq!(all.listed_pads.len(), 1, "Should have one root pad");
        let parent_dp = &all.listed_pads[0];
        assert_eq!(parent_dp.pad.metadata.title, "Parent");
        assert_eq!(parent_dp.children.len(), 1, "Parent should have one child");
        let child_dp = &parent_dp.children[0];
        assert_eq!(child_dp.pad.metadata.id, child_id);
        assert_eq!(
            child_dp.pad.metadata.parent_id, pad.metadata.parent_id,
            "Parent ID must survive the full create→editor→refresh→propagate cycle"
        );
    }
}
