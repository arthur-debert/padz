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

    let mut unarchived_uuids: Vec<Uuid> = Vec::new();
    let mut processed_ids = std::collections::HashSet::new();

    for (_display_index, uuid) in resolved {
        if !processed_ids.insert(uuid) {
            continue;
        }

        // Only unarchive pads that are in the Archived bucket
        let pad = match store.get_pad(&uuid, scope, Bucket::Archived) {
            Ok(p) => p,
            Err(_) => continue, // Skip if not in Archived (idempotent)
        };
        let parent_id = pad.metadata.parent_id;

        // Find descendants in the tree
        let descendants = super::helpers::get_descendant_ids(store, scope, &[uuid])?;

        // Move pad + all descendants from Archived to Active
        let mut ids_to_move = vec![uuid];
        ids_to_move.extend(&descendants);
        store.move_pads(&ids_to_move, scope, Bucket::Archived, Bucket::Active)?;

        for id in &descendants {
            processed_ids.insert(*id);
        }

        // If unarchiving a child whose parent is still in Archived, clear parent_id
        if let Some(pid) = parent_id {
            if !processed_ids.contains(&pid) && store.get_pad(&pid, scope, Bucket::Active).is_err()
            {
                let mut restored_pad = store.get_pad(&uuid, scope, Bucket::Active)?;
                restored_pad.metadata.parent_id = None;
                store.save_pad(&restored_pad, scope, Bucket::Active)?;
            }
        }

        // Propagate status change to parent (unarchived child affects status again)
        crate::todos::propagate_status_change(store, scope, parent_id)?;

        unarchived_uuids.push(uuid);
    }

    // Re-index to get the new regular indexes
    let indexed = indexed_pads(store, scope)?;
    for uuid in unarchived_uuids {
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
    use crate::commands::{archive, create, get};
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn unarchives_pad() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();

        // Archive it
        archive::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Unarchive it
        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Archived(1)])],
        )
        .unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert!(matches!(
            result.affected_pads[0].index,
            DisplayIndex::Regular(1)
        ));

        // Verify active again
        let active = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(active.listed_pads.len(), 1);

        // Verify archived is empty
        let archived = get::run(
            &store,
            Scope::Project,
            get::PadFilter {
                status: get::PadStatusFilter::Archived,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert_eq!(archived.listed_pads.len(), 0);
    }

    #[test]
    fn unarchive_parent_restores_children() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Archive parent (moves child too)
        archive::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Unarchive parent
        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Archived(1)])],
        )
        .unwrap();

        // Both should be active again
        let active = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(active.listed_pads.len(), 1);
        assert_eq!(active.listed_pads[0].pad.metadata.title, "Parent");
        assert_eq!(active.listed_pads[0].children.len(), 1);
    }

    #[test]
    fn unarchive_non_archived_is_noop() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        assert_eq!(result.affected_pads.len(), 0);
        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Active)
                .unwrap()
                .len(),
            1
        );
    }
}
