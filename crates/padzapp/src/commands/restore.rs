use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::Bucket;
use crate::store::DataStore;
use uuid::Uuid;

use super::helpers::{indexed_pads, resolve_selectors};

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    let resolved = resolve_selectors(store, scope, selectors, false)?;
    let mut result = CmdResult::default();

    let mut restored_uuids: Vec<Uuid> = Vec::new();
    let mut processed_ids = std::collections::HashSet::new();

    for (_display_index, uuid) in resolved {
        if !processed_ids.insert(uuid) {
            continue;
        }

        // Only restore pads that are in the Deleted bucket
        let pad = match store.get_pad(&uuid, scope, Bucket::Deleted) {
            Ok(p) => p,
            Err(_) => continue, // Skip if not in Deleted (idempotent)
        };
        let parent_id = pad.metadata.parent_id;

        // Find descendants in the tree (they're in the same Deleted bucket)
        let descendants = super::helpers::get_descendant_ids(store, scope, &[uuid])?;

        // Move pad + all descendants from Deleted to Active
        let mut ids_to_move = vec![uuid];
        ids_to_move.extend(&descendants);
        store.move_pads(&ids_to_move, scope, Bucket::Deleted, Bucket::Active)?;

        for id in &descendants {
            processed_ids.insert(*id);
        }

        // If restoring a child whose parent is still in Deleted, clear parent_id
        if let Some(pid) = parent_id {
            if !processed_ids.contains(&pid) && store.get_pad(&pid, scope, Bucket::Active).is_err()
            {
                let mut restored_pad = store.get_pad(&uuid, scope, Bucket::Active)?;
                restored_pad.metadata.parent_id = None;
                store.save_pad(&restored_pad, scope, Bucket::Active)?;
            }
        }

        // Propagate status change to parent (restored child affects status again)
        crate::todos::propagate_status_change(store, scope, parent_id)?;

        restored_uuids.push(uuid);
    }

    // Re-index to get the new regular indexes
    let indexed = indexed_pads(store, scope)?;
    for uuid in restored_uuids {
        if let Some(dp) = super::helpers::find_pad_by_uuid(&indexed, uuid, |idx| {
            matches!(idx, DisplayIndex::Regular(_) | DisplayIndex::Pinned(_))
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
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn restores_deleted_pad() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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
            &[],
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

        // No messages - CLI handles unified rendering
        assert!(result.messages.is_empty());

        // Verify affected_pads contains the restored pad
        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "Title");
        assert!(matches!(
            result.affected_pads[0].index,
            DisplayIndex::Regular(1)
        ));

        // Verify it's active again
        let active = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
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
            &[],
        )
        .unwrap();
        assert_eq!(deleted_after.listed_pads.len(), 0);
    }

    #[test]
    fn restores_multiple_pads() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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

        // No messages - CLI handles unified rendering
        assert!(result.messages.is_empty());
        // Should have 2 affected pads
        assert_eq!(result.affected_pads.len(), 2);

        // Verify 2 active, 1 deleted
        let active = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(active.listed_pads.len(), 2);

        let deleted = get::run(
            &store,
            Scope::Project,
            get::PadFilter {
                status: get::PadStatusFilter::Deleted,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert_eq!(deleted.listed_pads.len(), 1);
    }

    #[test]
    fn preserves_original_created_at() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create two pads with a small delay between them
        create::run(&mut store, Scope::Project, "Older".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Newer".into(), "".into(), None).unwrap();

        // Get original created_at of the older pad (which is now index 2 since newest first)
        let original = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
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

        // Verify created_at is preserved (bucket move doesn't alter metadata)
        let after = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        let restored_pad = after
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "Older")
            .unwrap();

        assert_eq!(restored_pad.pad.metadata.created_at, original_created_at);
    }

    #[test]
    fn restore_deleted_parent_makes_children_visible() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

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
        let active = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(active.listed_pads.len(), 0);

        // Restore parent
        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Deleted(1)])],
        )
        .unwrap();

        // Verify parent is active and child is visible again
        let active_after =
            get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
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
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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

        // No messages - CLI handles unified rendering
        assert!(result.messages.is_empty());
        // Should have 2 affected pads
        assert_eq!(result.affected_pads.len(), 2);
        let count = store
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .len();
        assert_eq!(count, 2);
    }

    #[test]
    fn restore_non_deleted_is_idempotent() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Restoring a pad that's already active (not in Deleted bucket) is a no-op
        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Pad was skipped (not in Deleted bucket), so no affected pads
        assert!(result.messages.is_empty());
        assert_eq!(result.affected_pads.len(), 0);

        // Pad is still active
        let active = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(active.listed_pads.len(), 1);
    }
}
