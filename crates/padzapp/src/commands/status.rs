//! # Todo Status Commands
//!
//! This module provides commands for managing the todo status of pads:
//! - [`complete`]: Marks pads as Done
//! - [`reopen`]: Reopens pads (sets them back to Planned)

use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::{Scope, TodoStatus};
use crate::store::{Bucket, DataStore};
use uuid::Uuid;

use super::helpers::{indexed_pads, resolve_selectors};

/// Marks pads as Done.
pub fn complete<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    set_status(store, scope, selectors, TodoStatus::Done)
}

/// Reopens pads (sets them back to Planned).
pub fn reopen<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    set_status(store, scope, selectors, TodoStatus::Planned)
}

fn set_status<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    new_status: TodoStatus,
) -> Result<CmdResult> {
    let resolved = resolve_selectors(store, scope, selectors, false)?;
    let mut result = CmdResult::default();

    let mut affected_uuids: Vec<Uuid> = Vec::new();
    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope, Bucket::Active)?;
        let old_status = pad.metadata.status;

        if old_status == new_status {
            // Already in desired state - add info message
            let status_name = match new_status {
                TodoStatus::Done => "done",
                TodoStatus::Planned => "planned",
                TodoStatus::InProgress => "in progress",
            };
            result.add_message(CmdMessage::info(format!(
                "Pad {} is already {}",
                super::helpers::fmt_path(&display_index),
                status_name
            )));
        } else {
            pad.metadata.status = new_status;
            pad.metadata.updated_at = chrono::Utc::now();

            let parent_id = pad.metadata.parent_id;
            store.save_pad(&pad, scope, Bucket::Active)?;

            // Propagate status change to parent
            crate::todos::propagate_status_change(store, scope, parent_id)?;

            // Note: No success message - CLI handles unified rendering via pad list
        }
        affected_uuids.push(uuid);
    }

    // Re-index to get the current indexes
    let indexed = indexed_pads(store, scope)?;
    for uuid in affected_uuids {
        // Prefer Regular index for active pads
        let index_filter =
            |idx: &DisplayIndex| matches!(idx, DisplayIndex::Regular(_) | DisplayIndex::Pinned(_));
        if let Some(dp) = super::helpers::find_pad_by_uuid(&indexed, uuid, index_filter) {
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
    use std::slice;

    #[test]
    fn complete_marks_pad_as_done() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Task".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        let result = complete(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        // No messages for successful completion - CLI handles unified rendering
        assert!(result.messages.is_empty());
        // Should have affected pad
        assert_eq!(result.affected_pads.len(), 1);

        let pads = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(pads.listed_pads[0].pad.metadata.status, TodoStatus::Done);
    }

    #[test]
    fn reopen_sets_pad_to_planned() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Task".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        // First complete it
        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        complete(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        // Then reopen it
        let result = reopen(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        // No messages for successful reopen - CLI handles unified rendering
        assert!(result.messages.is_empty());
        // Should have affected pad
        assert_eq!(result.affected_pads.len(), 1);

        let pads = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(pads.listed_pads[0].pad.metadata.status, TodoStatus::Planned);
    }

    #[test]
    fn complete_already_done_is_idempotent() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Task".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        complete(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        // Complete again
        let result = complete(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        assert!(matches!(
            result.messages[0].level,
            crate::commands::MessageLevel::Info
        ));
        assert!(result.messages[0].content.contains("already done"));
    }

    #[test]
    fn reopen_already_planned_is_idempotent() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Task".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);

        // Reopen an already planned pad
        let result = reopen(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        assert!(matches!(
            result.messages[0].level,
            crate::commands::MessageLevel::Info
        ));
        assert!(result.messages[0].content.contains("already planned"));
    }

    #[test]
    fn complete_batch() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "A".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "B".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        let selectors = vec![
            PadSelector::Path(vec![DisplayIndex::Regular(1)]),
            PadSelector::Path(vec![DisplayIndex::Regular(2)]),
        ];

        let result = complete(&mut store, Scope::Project, &selectors).unwrap();

        // No messages for successful completion - CLI handles unified rendering
        assert!(result.messages.is_empty());
        // Should have 2 affected pads
        assert_eq!(result.affected_pads.len(), 2);

        let pads = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert!(pads
            .listed_pads
            .iter()
            .all(|dp| dp.pad.metadata.status == TodoStatus::Done));
    }

    #[test]
    fn complete_propagates_to_parent() {
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

        // Create child
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
            Vec::new(),
        )
        .unwrap();

        // Complete the child (1.1)
        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1), DisplayIndex::Regular(1)]);
        complete(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        // Parent should now be Done (all children are Done)
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let parent = pads.iter().find(|p| p.metadata.title == "Parent").unwrap();
        assert_eq!(parent.metadata.status, TodoStatus::Done);
    }

    #[test]
    fn reopen_propagates_to_parent() {
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

        // Create child
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
            Vec::new(),
        )
        .unwrap();

        // Complete the child
        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1), DisplayIndex::Regular(1)]);
        complete(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        // Verify parent is Done
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let parent = pads.iter().find(|p| p.metadata.title == "Parent").unwrap();
        assert_eq!(parent.metadata.status, TodoStatus::Done);

        // Reopen the child
        reopen(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        // Parent should now be Planned (all children are Planned)
        let pads_after = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let parent_after = pads_after
            .iter()
            .find(|p| p.metadata.title == "Parent")
            .unwrap();
        assert_eq!(parent_after.metadata.status, TodoStatus::Planned);
    }
}
