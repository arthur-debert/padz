use crate::attributes::AttrValue;
use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::{Bucket, DataStore};
use uuid::Uuid;

use super::helpers::{indexed_pads, resolve_selectors};

pub fn pin<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    pin_state(store, scope, selectors, true)
}

pub fn unpin<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    pin_state(store, scope, selectors, false)
}

fn pin_state<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    is_pinned: bool,
) -> Result<CmdResult> {
    let resolved = resolve_selectors(store, scope, selectors, false)?;
    let mut result = CmdResult::default();

    // Collect UUIDs and perform pin/unpin
    let mut affected_uuids: Vec<Uuid> = Vec::new();
    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope, Bucket::Active)?;
        let was_already_pinned = pad.metadata.is_pinned; // Capture original state

        // Use the attribute API - this sets is_pinned, pinned_at, and delete_protected
        pad.metadata.set_attr("pinned", AttrValue::Bool(is_pinned));
        store.save_pad(&pad, scope, Bucket::Active)?;

        // Only add info messages for no-op cases; success cases are shown via pad list
        if is_pinned && was_already_pinned {
            result.add_message(CmdMessage::info(format!(
                "Pad {} is already pinned",
                super::helpers::fmt_path(&display_index)
            )));
        } else if !is_pinned && !was_already_pinned {
            result.add_message(CmdMessage::info(format!(
                "Pad {} is already unpinned",
                super::helpers::fmt_path(&display_index)
            )));
        }
        affected_uuids.push(uuid);
    }

    // Re-index to get the new indexes (pinned pads get pN index)
    let indexed = indexed_pads(store, scope)?;
    for uuid in affected_uuids {
        // For pinned pads, prefer Pinned index; for unpinned, prefer Regular
        let index_filter = if is_pinned {
            |idx: &DisplayIndex| matches!(idx, DisplayIndex::Pinned(_))
        } else {
            |idx: &DisplayIndex| matches!(idx, DisplayIndex::Regular(_))
        };
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
    fn pinning_assigns_p_index() {
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

        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        pin(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        let result = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert!(result
            .listed_pads
            .iter()
            .any(|dp| matches!(dp.index, DisplayIndex::Pinned(1))));
    }

    #[test]
    fn unpinning_removes_pinned_flag() {
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
        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        pin(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();
        unpin(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        let result = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert!(result
            .listed_pads
            .iter()
            .all(|dp| !matches!(dp.index, DisplayIndex::Pinned(_))));
    }

    #[test]
    fn pinning_enables_delete_protection() {
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

        // Pin it
        pin(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Check if protected
        let pads = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert!(pads.listed_pads[0].pad.metadata.delete_protected);

        // Try to delete (should fail)
        use crate::commands::delete;
        let err = delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Pinned(1)])],
        );
        assert!(err.is_err());

        // Unpin it
        unpin(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Pinned(1)])],
        )
        .unwrap();

        // Check is unprotected
        let pads_after = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert!(!pads_after.listed_pads[0].pad.metadata.delete_protected);

        // Try to delete (should succeed)
        let success = delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        );
        assert!(success.is_ok());
    }

    #[test]
    fn pinning_already_pinned_is_idempotent() {
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

        // Pin first time
        pin(
            &mut store,
            Scope::Project,
            slice::from_ref(&PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Pin second time
        let result = pin(
            &mut store,
            Scope::Project,
            slice::from_ref(&PadSelector::Path(vec![DisplayIndex::Pinned(1)])),
        )
        .unwrap();

        // Should return info message
        assert!(matches!(
            result.messages[0].level,
            crate::commands::MessageLevel::Info
        ));
        assert!(result.messages[0].content.contains("already pinned"));
    }

    #[test]
    fn unpinning_already_unpinned_is_idempotent() {
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

        // Unpin unpinned
        let result = unpin(
            &mut store,
            Scope::Project,
            slice::from_ref(&PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Should return info message
        assert!(matches!(
            result.messages[0].level,
            crate::commands::MessageLevel::Info
        ));
        assert!(result.messages[0].content.contains("already unpinned"));
    }

    #[test]
    fn pinning_batch() {
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

        pin(&mut store, Scope::Project, &selectors).unwrap();

        let pads = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        // Both pads should be pinned.
        for dp in &pads.listed_pads {
            assert!(
                dp.pad.metadata.is_pinned,
                "Pad {} should be is_pinned=true",
                dp.pad.metadata.title
            );
        }
        // When a pad is pinned, it appears twice in the default listing:
        // Once as Regular(X) and once as Pinned(Y).
        assert_eq!(pads.listed_pads.len(), 4);

        let pinned_count = pads
            .listed_pads
            .iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Pinned(_)))
            .count();
        let regular_count = pads
            .listed_pads
            .iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Regular(_)))
            .count();

        assert_eq!(pinned_count, 2);
        assert_eq!(regular_count, 2);
    }
}
