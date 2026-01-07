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
    let resolved = resolve_selectors(store, scope, selectors, false)?;
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
            display_index, pad.metadata.title
        )));
        restored_uuids.push(uuid);
    }

    // Re-index to get the new regular indexes
    let indexed = indexed_pads(store, scope)?;
    for uuid in restored_uuids {
        // Restored pads get Regular index
        let dp = indexed
            .iter()
            .filter(|dp| dp.pad.metadata.id == uuid)
            .find(|dp| matches!(dp.index, DisplayIndex::Regular(_)));

        if let Some(dp) = dp {
            result.affected_pads.push(DisplayPad {
                pad: dp.pad.clone(),
                index: dp.index.clone(),
                matches: None,
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
        create::run(&mut store, Scope::Project, "Title".into(), "".into()).unwrap();

        // Delete it
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Index(DisplayIndex::Regular(1))],
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
            &[PadSelector::Index(DisplayIndex::Deleted(1))],
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
        create::run(&mut store, Scope::Project, "A".into(), "".into()).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into()).unwrap();
        create::run(&mut store, Scope::Project, "C".into(), "".into()).unwrap();

        // Delete all three
        delete::run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Index(DisplayIndex::Regular(1)),
                PadSelector::Index(DisplayIndex::Regular(2)),
                PadSelector::Index(DisplayIndex::Regular(3)),
            ],
        )
        .unwrap();

        // Restore two of them
        let result = run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Index(DisplayIndex::Deleted(1)),
                PadSelector::Index(DisplayIndex::Deleted(3)),
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
        create::run(&mut store, Scope::Project, "Older".into(), "".into()).unwrap();
        create::run(&mut store, Scope::Project, "Newer".into(), "".into()).unwrap();

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
            &[PadSelector::Index(DisplayIndex::Regular(2))],
        )
        .unwrap();

        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Index(DisplayIndex::Deleted(1))],
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
}
