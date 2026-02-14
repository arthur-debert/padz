use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::{DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::{Bucket, DataStore};
use uuid::Uuid;

use super::helpers::{indexed_pads, resolve_selectors};

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    let resolved = resolve_selectors(store, scope, selectors, true)?;
    let mut result = CmdResult::default();

    let mut deleted_uuids: Vec<Uuid> = Vec::new();
    let mut processed_ids = std::collections::HashSet::new();

    for (_display_index, uuid) in resolved {
        if !processed_ids.insert(uuid) {
            continue; // Already processed (e.g., as a descendant of an earlier pad)
        }

        // Get the pad's parent before moving (parent stays in Active)
        let pad = store.get_pad(&uuid, scope, Bucket::Active)?;
        let parent_id = pad.metadata.parent_id;

        // Find descendants (children, grandchildren, etc.)
        let descendants = super::helpers::get_descendant_ids(store, scope, &[uuid])?;

        // Move pad + all descendants from Active to Deleted
        let mut ids_to_move = vec![uuid];
        ids_to_move.extend(&descendants);
        store.move_pads(&ids_to_move, scope, Bucket::Active, Bucket::Deleted)?;

        for id in &descendants {
            processed_ids.insert(*id);
        }

        // Propagate status change to parent (deleted child no longer affects parent status)
        crate::todos::propagate_status_change(store, scope, parent_id)?;

        deleted_uuids.push(uuid);
    }

    // Re-index to get the new deleted indexes
    let indexed = indexed_pads(store, scope)?;
    for uuid in deleted_uuids {
        if let Some(dp) = super::helpers::find_pad_by_uuid(&indexed, uuid, |_| true) {
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
    use crate::commands::{create, get};
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn marks_pad_as_deleted() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Title".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();
        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

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
    }

    #[test]
    fn delete_protected_pad_fails() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Protected".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        // Manually protect the pad (since pin command logic isn't coupled yet or might not be updated yet)
        let pad_id = get::run(&store, Scope::Project, get::PadFilter::default(), &[])
            .unwrap()
            .listed_pads[0]
            .pad
            .metadata
            .id;

        let mut pad = store
            .get_pad(&pad_id, Scope::Project, Bucket::Active)
            .unwrap();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        // Attempt delete
        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        );

        assert!(result.is_err());
        match result {
            Err(crate::error::PadzError::Api(msg)) => {
                assert!(msg.contains("Pinned pads are delete protected"));
            }
            _ => panic!("Expected Api error"),
        }
    }

    #[test]
    fn delete_parent_with_pinned_child_succeeds() {
        // Deleting a parent should work even if it has a pinned child.
        // The pinned child is NOT deleted (soft delete is non-recursive per spec).
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create parent
        create::run(
            &mut store,
            Scope::Project,
            "Parent".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        // Create child inside parent
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
            Vec::new(),
        )
        .unwrap();

        // Pin the child (1.1)
        crate::commands::pinning::pin(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        )
        .unwrap();

        // Delete the parent - should succeed (parent is not pinned)
        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        );
        assert!(result.is_ok());

        // Verify parent is deleted
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
        assert_eq!(deleted.listed_pads[0].pad.metadata.title, "Parent");

        // Child moves to Deleted bucket with parent — no dual pinned indexing in Deleted
        assert_eq!(deleted.listed_pads[0].children.len(), 1);
    }

    #[test]
    fn delete_nested_pad_via_path() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create parent
        create::run(
            &mut store,
            Scope::Project,
            "Parent".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        // Create child inside parent
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
            Vec::new(),
        )
        .unwrap();

        // Delete the child using path notation 1.1
        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        );
        assert!(result.is_ok());

        // Parent should still be active with no visible children
        let active = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(active.listed_pads.len(), 1);
        assert_eq!(active.listed_pads[0].pad.metadata.title, "Parent");
        assert_eq!(active.listed_pads[0].children.len(), 0); // child is deleted
    }
}
