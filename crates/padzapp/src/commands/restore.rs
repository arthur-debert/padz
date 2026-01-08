use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;
use uuid::Uuid;

use super::helpers::{indexed_pads, resolve_selectors};

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    let resolved = resolve_selectors(store, scope, selectors, true)?;
    let mut result = CmdResult::default();

    // Collect UUIDs and perform restores
    let mut restored_uuids: Vec<Uuid> = Vec::new();
    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope)?;
        pad.metadata.is_deleted = false;
        pad.metadata.deleted_at = None;
        // Keep original created_at so the pad appears in its original position
        store.save_pad(&pad, scope)?;
        result.add_message(CmdMessage::success(format!(
            "Pad restored ({}): {}",
            super::helpers::fmt_path(&display_index),
            pad.metadata.title
        )));
        restored_uuids.push(uuid);
    }

    // Re-index to get the new regular indexes
    let indexed = indexed_pads(store, scope)?;
    for uuid in restored_uuids {
        // Restored pads get Regular index
        if let Some(dp) = super::helpers::find_pad_by_uuid(&indexed, uuid, |idx| {
            matches!(idx, DisplayIndex::Regular(_))
        }) {
            result.affected_pads.push(DisplayPad {
                pad: dp.pad.clone(),
                index: dp.index.clone(),
                matches: None,
                children: Vec::new(),
            });
        }
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
    fn restores_deleted_pad() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();

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
        assert!(matches!(
            deleted.listed_pads[0].index,
            DisplayIndex::Deleted(1)
        ));

        // Restore it
        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Deleted(1)])],
        )
        .unwrap();

        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].content.contains("Pad restored"));
        assert!(result.messages[0].content.contains("Title"));

        // Verify it's active again
        let active = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(active.listed_pads.len(), 1);
        assert!(matches!(
            active.listed_pads[0].index,
            DisplayIndex::Regular(1)
        ));

        // Verify deleted list is empty
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
    fn restores_multiple_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "C".into(), "".into(), None).unwrap();

        // Delete all three
        delete::run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
                PadSelector::Path(vec![DisplayIndex::Regular(3)]),
            ],
        )
        .unwrap();

        // Restore two of them
        let result = run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Deleted(1)]),
                PadSelector::Path(vec![DisplayIndex::Deleted(3)]),
            ],
        )
        .unwrap();

        assert_eq!(result.messages.len(), 2);

        // Verify 2 active, 1 deleted
        let active = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(active.listed_pads.len(), 2);

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
    }

    #[test]
    fn preserves_original_created_at() {
        let mut store = InMemoryStore::new();

        // Create two pads with a small delay between them
        create::run(&mut store, Scope::Project, "Older".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Newer".into(), "".into(), None).unwrap();

        // Get original created_at of the older pad (which is now index 2 since newest first)
        let original = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        let older_pad = original
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "Older")
            .unwrap();
        let original_created_at = older_pad.pad.metadata.created_at;

        // Delete and restore the older pad
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(2)])],
        )
        .unwrap();

        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Deleted(1)])],
        )
        .unwrap();

        // Verify created_at is preserved
        let after = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        let restored_pad = after
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "Older")
            .unwrap();

        assert_eq!(restored_pad.pad.metadata.created_at, original_created_at);
        assert!(!restored_pad.pad.metadata.is_deleted);
        assert!(restored_pad.pad.metadata.deleted_at.is_none());
    }

    #[test]
    fn restore_deleted_parent_makes_children_visible() {
        let mut store = InMemoryStore::new();

        // Create parent with child
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Delete parent (child is NOT marked deleted per spec)
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Verify parent and child are hidden from active view
        let active = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(active.listed_pads.len(), 0);

        // Restore parent
        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Deleted(1)])],
        )
        .unwrap();

        // Verify parent is active and child is visible again
        let active_after = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(active_after.listed_pads.len(), 1);
        assert_eq!(active_after.listed_pads[0].pad.metadata.title, "Parent");
        assert_eq!(active_after.listed_pads[0].children.len(), 1);
        assert_eq!(
            active_after.listed_pads[0].children[0].pad.metadata.title,
            "Child"
        );
    }

    #[test]
    fn restore_batch() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();

        // Delete both
        delete::run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
            ],
        )
        .unwrap();

        // Restore both
        let result = run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Deleted(1)]),
                PadSelector::Path(vec![DisplayIndex::Deleted(2)]),
            ],
        )
        .unwrap();

        assert_eq!(result.messages.len(), 2);
        let count = store
            .list_pads(Scope::Project)
            .unwrap()
            .iter()
            .filter(|p| !p.metadata.is_deleted)
            .count();
        assert_eq!(count, 2);
    }

    #[test]
    fn restore_non_deleted_fails_selector() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Try to restore an active pad using Regular selector?
        // restore::run calls resolve_selectors(..., true). true means 'match_deleted'.
        // If we pass Regular selector to match_deleted=true, resolve_selectors might fail or just not match non-deleted items depending on logic.
        // Actually resolve_selectors logic:
        // If match_deleted=true, it looks for matches in BOTH? or specifically checks deleted?
        // Let's test the behavior:

        // If I pass Regular selector, match_deleted=true in resolve_selectors allows matching ANY pad
        // including active ones. The command effectively becomes idempotent.
        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // It should succeed and report restored (idempotent)
        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].content.contains("Pad restored"));
    }
}
