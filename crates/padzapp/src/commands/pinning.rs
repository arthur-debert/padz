use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;

use super::helpers::resolve_selectors;

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

    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope)?;
        let was_already_pinned = pad.metadata.is_pinned; // Capture original state

        pad.metadata.is_pinned = is_pinned;
        pad.metadata.pinned_at = if is_pinned { Some(Utc::now()) } else { None };
        pad.metadata.delete_protected = is_pinned;
        store.save_pad(&pad, scope)?;

        if is_pinned && !was_already_pinned {
            // Check if it was actually pinned
            result.add_message(CmdMessage::success(format!(
                "Pinned pad {}",
                super::helpers::fmt_path(&display_index)
            )));
        } else if !is_pinned && was_already_pinned {
            // Check if it was actually unpinned
            result.add_message(CmdMessage::success(format!(
                "Unpinned pad {}",
                super::helpers::fmt_path(&display_index)
            )));
        } else if is_pinned && was_already_pinned {
            result.add_message(CmdMessage::info(format!(
                "Pad {} is already pinned",
                super::helpers::fmt_path(&display_index)
            )));
        } else {
            // !is_pinned && !was_already_pinned
            result.add_message(CmdMessage::info(format!(
                "Pad {} is already unpinned",
                super::helpers::fmt_path(&display_index)
            )));
        }
        result.affected_pads.push(pad);
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
    use std::slice;

    #[test]
    fn pinning_assigns_p_index() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();

        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        pin(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        let result = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert!(result
            .listed_pads
            .iter()
            .any(|dp| matches!(dp.index, DisplayIndex::Pinned(1))));
    }

    #[test]
    fn unpinning_removes_pinned_flag() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        let sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        pin(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();
        unpin(&mut store, Scope::Project, slice::from_ref(&sel)).unwrap();

        let result = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert!(result
            .listed_pads
            .iter()
            .all(|dp| !matches!(dp.index, DisplayIndex::Pinned(_))));
    }

    #[test]
    fn pinning_enables_delete_protection() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Pin it
        pin(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Check if protected
        let pads = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
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
        let pads_after = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert!(!pads_after.listed_pads[0].pad.metadata.delete_protected);

        // Try to delete (should succeed)
        let success = delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        );
        assert!(success.is_ok());
    }
}
