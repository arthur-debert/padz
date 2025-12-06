use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::index::DisplayIndex;
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;

use super::helpers::resolve_indexes;

pub fn pin<S: DataStore>(
    store: &mut S,
    scope: Scope,
    indexes: &[DisplayIndex],
) -> Result<CmdResult> {
    pin_state(store, scope, indexes, true)
}

pub fn unpin<S: DataStore>(
    store: &mut S,
    scope: Scope,
    indexes: &[DisplayIndex],
) -> Result<CmdResult> {
    pin_state(store, scope, indexes, false)
}

fn pin_state<S: DataStore>(
    store: &mut S,
    scope: Scope,
    indexes: &[DisplayIndex],
    is_pinned: bool,
) -> Result<CmdResult> {
    let resolved = resolve_indexes(store, scope, indexes)?;
    let mut result = CmdResult::default();

    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope)?;
        pad.metadata.is_pinned = is_pinned;
        pad.metadata.pinned_at = if is_pinned { Some(Utc::now()) } else { None };
        store.save_pad(&pad, scope)?;

        let verb = if is_pinned { "pinned" } else { "unpinned" };
        result.add_message(CmdMessage::success(format!(
            "Pad {} ({}): {}",
            verb, display_index, pad.metadata.title
        )));
        result.affected_pads.push(pad);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, list};
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;
    use std::slice;

    #[test]
    fn pinning_assigns_p_index() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into()).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into()).unwrap();

        let idx = DisplayIndex::Regular(1);
        pin(&mut store, Scope::Project, slice::from_ref(&idx)).unwrap();

        let result = list::run(&store, Scope::Project, false).unwrap();
        assert!(result
            .listed_pads
            .iter()
            .any(|dp| matches!(dp.index, DisplayIndex::Pinned(1))));
    }

    #[test]
    fn unpinning_removes_pinned_flag() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into()).unwrap();
        let idx = DisplayIndex::Regular(1);
        pin(&mut store, Scope::Project, slice::from_ref(&idx)).unwrap();
        unpin(&mut store, Scope::Project, slice::from_ref(&idx)).unwrap();

        let result = list::run(&store, Scope::Project, false).unwrap();
        assert!(result
            .listed_pads
            .iter()
            .all(|dp| !matches!(dp.index, DisplayIndex::Pinned(_))));
    }
}
