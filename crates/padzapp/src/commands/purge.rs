use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;
use uuid::Uuid;

use super::helpers::{indexed_pads, pads_by_selectors};

/// Preview of what a purge operation would delete.
/// Used by CLI to show confirmation before executing.
#[derive(Debug)]
pub struct PurgePreview {
    /// Pads directly targeted for deletion
    pub targets: Vec<DisplayPad>,
    /// Number of descendants that will also be deleted
    pub descendant_count: usize,
}

/// Returns a preview of what would be purged, without actually deleting anything.
///
/// Use this to show a confirmation prompt in the CLI before calling `run`.
pub fn preview<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    recursive: bool,
) -> Result<PurgePreview> {
    // 1. Resolve targets
    let pads_to_purge = if selectors.is_empty() {
        let all_pads = indexed_pads(store, scope)?;
        all_pads
            .into_iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Deleted(_)))
            .collect()
    } else {
        pads_by_selectors(store, scope, selectors, true)?
    };

    // 2. Find descendants
    let target_ids: Vec<Uuid> = pads_to_purge.iter().map(|dp| dp.pad.metadata.id).collect();
    let descendants = super::helpers::get_descendant_ids(store, scope, &target_ids)?;

    // 3. Safety valve: require --recursive if there are children
    if !descendants.is_empty() && !recursive {
        return Err(PadzError::Api(format!(
            "Cannot purge: {} pad(s) have children. Use --recursive (-r) to purge entire subtrees.",
            pads_to_purge
                .iter()
                .filter(|dp| {
                    let id = dp.pad.metadata.id;
                    super::helpers::get_descendant_ids(store, scope, &[id])
                        .map(|d| !d.is_empty())
                        .unwrap_or(false)
                })
                .count()
        )));
    }

    Ok(PurgePreview {
        targets: pads_to_purge,
        descendant_count: descendants.len(),
    })
}

/// Permanently removes pads from the store.
///
/// **Safety valve**: When purging pads that have children, the `recursive` flag must be set.
/// This prevents accidental deletion of entire subtrees.
///
/// **Important**: This function does NOT prompt for confirmation. The CLI layer should
/// call `preview()` first, show confirmation to the user, then call this function.
///
/// - If `selectors` is empty, targets all deleted pads
/// - If `recursive` is false and any target has children, returns an error
pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    recursive: bool,
) -> Result<CmdResult> {
    // Get the preview (also validates recursive flag)
    let preview = preview(store, scope, selectors, recursive)?;

    if preview.targets.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to purge."));
        return Ok(res);
    }

    // Collect all IDs to delete
    let target_ids: Vec<Uuid> = preview
        .targets
        .iter()
        .map(|dp| dp.pad.metadata.id)
        .collect();
    let descendants = super::helpers::get_descendant_ids(store, scope, &target_ids)?;

    let mut all_ids = target_ids;
    all_ids.extend(descendants.clone());
    all_ids.sort();
    all_ids.dedup();

    // Delete ALL
    let mut result = CmdResult::default();
    for id in all_ids {
        if store.get_pad(&id, scope).is_ok() {
            store.delete_pad(&id, scope)?;
        }
    }

    for dp in preview.targets {
        result.add_message(CmdMessage::success(format!(
            "Purged: {} {}",
            dp.index, dp.pad.metadata.title
        )));
    }
    if !descendants.is_empty() {
        result.add_message(CmdMessage::success(format!(
            "And purged {} descendants",
            descendants.len()
        )));
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, delete, get};
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn purges_deleted_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Delete it
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Verify it's deleted
        let deleted = get::run(
            &store,
            Scope::Project,
            get::PadFilter {
                status: get::PadStatusFilter::Deleted,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(deleted.listed_pads.len(), 1);

        // Purge (no recursive needed - pad has no children)
        let res = run(
            &mut store,
            Scope::Project,
            &[],
            false, // recursive not needed
        )
        .unwrap();

        assert_eq!(res.messages.len(), 1);
        assert!(res.messages[0].content.contains("Purged: d1 A"));

        // Verify empty
        let deleted_after = get::run(
            &store,
            Scope::Project,
            get::PadFilter {
                status: get::PadStatusFilter::Deleted,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(deleted_after.listed_pads.len(), 0);
    }

    #[test]
    fn purges_specific_pads_even_if_active() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Purge active pad 1 (no children, so recursive not needed)
        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            false,
        )
        .unwrap();

        assert_eq!(res.messages.len(), 1);
        assert!(res.messages[0].content.contains("Purged: 1 A"));

        // Verify gone
        let listed = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(listed.listed_pads.len(), 0);
    }

    #[test]
    fn does_nothing_if_no_deleted_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Purge deleted (none)
        let res = run(&mut store, Scope::Project, &[], false).unwrap();

        assert_eq!(res.messages.len(), 1);
        assert!(res.messages[0].content.contains("No pads to purge"));

        // A still exists
        let listed = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
    }

    #[test]
    fn purges_recursively_with_flag() {
        let mut store = InMemoryStore::new();
        // Create Parent
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        // Create Child inside Parent (id=1)
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Delete Parent
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Purge Parent WITH recursive flag
        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Deleted(1)])],
            true, // recursive = true
        )
        .unwrap();

        assert!(res.messages[0].content.contains("Purged: d1 Parent"));
        // Check for descendant message
        let has_descendant_msg = res
            .messages
            .iter()
            .any(|m| m.content.contains("And purged 1 descendants"));
        assert!(has_descendant_msg);

        // Verify Store is empty
        let all_pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(all_pads.len(), 0);
    }

    #[test]
    fn purge_without_recursive_fails_when_has_children() {
        let mut store = InMemoryStore::new();
        // Create Parent
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        // Create Child inside Parent
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Try to purge Parent WITHOUT recursive flag - should fail
        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            false, // recursive = false
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("have children"));
        assert!(err.to_string().contains("--recursive"));

        // Verify nothing was deleted
        let all_pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(all_pads.len(), 2);
    }

    #[test]
    fn preview_returns_targets_and_descendants() {
        let mut store = InMemoryStore::new();
        // Create Parent
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        // Create Child
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Preview purge with recursive
        let preview_result = preview(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            true,
        )
        .unwrap();

        assert_eq!(preview_result.targets.len(), 1);
        assert_eq!(preview_result.targets[0].pad.metadata.title, "Parent");
        assert_eq!(preview_result.descendant_count, 1);
    }

    #[test]
    fn purge_selectors_vs_all() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();

        // Delete both to make them purgeable candidates
        delete::run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
            ],
        )
        .unwrap();

        // Purge only one (selectors provided)
        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Deleted(1)])],
            false,
        )
        .unwrap();

        assert_eq!(res.messages.len(), 1); // "Purged: ..."

        let remaining = store.list_pads(Scope::Project).unwrap();
        assert_eq!(remaining.len(), 1); // One remains
        assert!(remaining[0].metadata.is_deleted);
    }

    #[test]
    fn purge_nothing_found() {
        let mut store = InMemoryStore::new();
        // Empty store
        let res = run(&mut store, Scope::Project, &[], false).unwrap();
        assert!(res.messages[0].content.contains("No pads to purge"));
    }
}
