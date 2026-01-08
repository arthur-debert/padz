use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::index::{DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;
use uuid::Uuid;

use super::helpers::{indexed_pads, resolve_selectors};

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    let resolved = resolve_selectors(store, scope, selectors, true)?;
    let mut result = CmdResult::default();

    // Collect UUIDs and perform deletions
    let mut deleted_uuids: Vec<Uuid> = Vec::new();
    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope)?;
        pad.metadata.is_deleted = true;
        pad.metadata.deleted_at = Some(Utc::now());
        store.save_pad(&pad, scope)?;

        // Propagate status change to parent (deleted child no longer affects status)
        crate::todos::propagate_status_change(store, scope, pad.metadata.parent_id)?;

        result.add_message(CmdMessage::success(format!(
            "Pad deleted ({}): {}",
            super::helpers::fmt_path(&display_index),
            pad.metadata.title
        )));
        deleted_uuids.push(uuid);
    }

    // Re-index to get the new deleted indexes
    let indexed = indexed_pads(store, scope)?;
    for uuid in deleted_uuids {
        // Find the pad with any index type (deleted pads may have Deleted index)
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
    use crate::store::memory::InMemoryStore;

    #[test]
    fn marks_pad_as_deleted() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();
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
        let mut store = InMemoryStore::new();
        create::run(
            &mut store,
            Scope::Project,
            "Protected".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Manually protect the pad (since pin command logic isn't coupled yet or might not be updated yet)
        let pad_id = get::run(&store, Scope::Project, get::PadFilter::default())
            .unwrap()
            .listed_pads[0]
            .pad
            .metadata
            .id;

        let mut pad = store.get_pad(&pad_id, Scope::Project).unwrap();
        pad.metadata.delete_protected = true;
        store.save_pad(&pad, Scope::Project).unwrap();

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
        let mut store = InMemoryStore::new();

        // Create parent
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();

        // Create child inside parent
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
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
        )
        .unwrap();
        assert_eq!(deleted.listed_pads.len(), 1);
        assert_eq!(deleted.listed_pads[0].pad.metadata.title, "Parent");

        // Child should still exist (not deleted, just hidden under deleted parent)
        // It appears under the deleted parent in deleted view
        assert_eq!(deleted.listed_pads[0].children.len(), 2); // pinned + regular entry
    }

    #[test]
    fn delete_nested_pad_via_path() {
        let mut store = InMemoryStore::new();

        // Create parent
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();

        // Create child inside parent
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
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
        let active = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(active.listed_pads.len(), 1);
        assert_eq!(active.listed_pads[0].pad.metadata.title, "Parent");
        assert_eq!(active.listed_pads[0].children.len(), 0); // child is deleted
    }
}
