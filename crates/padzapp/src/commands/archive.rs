use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::{DisplayPad, PadSelector};
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

    let mut archived_uuids: Vec<Uuid> = Vec::new();
    let mut processed_ids = std::collections::HashSet::new();

    for (_display_index, uuid) in resolved {
        if !processed_ids.insert(uuid) {
            continue;
        }

        let pad = store.get_pad(&uuid, scope, Bucket::Active)?;
        let parent_id = pad.metadata.parent_id;

        // Find descendants (children, grandchildren, etc.)
        let descendants = super::helpers::get_descendant_ids(store, scope, &[uuid])?;

        // Move pad + all descendants from Active to Archived
        let mut ids_to_move = vec![uuid];
        ids_to_move.extend(&descendants);
        store.move_pads(&ids_to_move, scope, Bucket::Active, Bucket::Archived)?;

        for id in &descendants {
            processed_ids.insert(*id);
        }

        // Propagate status change to parent (archived child no longer affects parent status)
        crate::todos::propagate_status_change(store, scope, parent_id)?;

        archived_uuids.push(uuid);
    }

    // Re-index to get the new archived indexes
    let indexed = indexed_pads(store, scope)?;
    for uuid in archived_uuids {
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
    fn archives_pad() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();
        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

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
        assert_eq!(archived.listed_pads.len(), 1);
        assert!(matches!(
            archived.listed_pads[0].index,
            DisplayIndex::Archived(1)
        ));

        // Active should be empty
        let active = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(active.listed_pads.len(), 0);
    }

    #[test]
    fn archive_parent_moves_children() {
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

        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Both should be archived
        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Active)
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Archived)
                .unwrap()
                .len(),
            2
        );
    }

    #[test]
    fn archive_nested_pad_via_path() {
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

        // Archive just the child
        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        )
        .unwrap();

        // Parent still active, child archived
        let active = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(active.listed_pads.len(), 1);
        assert_eq!(active.listed_pads[0].pad.metadata.title, "Parent");
        assert_eq!(active.listed_pads[0].children.len(), 0);

        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Archived)
                .unwrap()
                .len(),
            1
        );
    }
}
