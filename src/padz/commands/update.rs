use crate::commands::{CmdMessage, CmdResult, PadUpdate};
use crate::error::Result;
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;

use super::helpers::resolve_indexes;

pub fn run<S: DataStore>(store: &mut S, scope: Scope, updates: &[PadUpdate]) -> Result<CmdResult> {
    if updates.is_empty() {
        return Ok(CmdResult::default());
    }

    let indexes: Vec<_> = updates.iter().map(|u| u.index.clone()).collect();
    let resolved = resolve_indexes(store, scope, &indexes)?;
    let mut result = CmdResult::default();

    for ((display_index, uuid), update) in resolved.into_iter().zip(updates.iter()) {
        let mut pad = store.get_pad(&uuid, scope)?;
        pad.metadata.title = update.title.clone();
        pad.metadata.updated_at = Utc::now();
        pad.content = update.content.clone();
        store.save_pad(&pad, scope)?;

        result.add_message(CmdMessage::success(format!(
            "Pad updated ({}): {}",
            display_index, pad.metadata.title
        )));
        result.affected_pads.push(pad);
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, view};
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn updates_pad_content() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "Old".into()).unwrap();
        let update = PadUpdate::new(DisplayIndex::Regular(1), "Title".into(), "New".into());
        run(&mut store, Scope::Project, &[update]).unwrap();

        let pads = view::run(&store, Scope::Project, &[DisplayIndex::Regular(1)])
            .unwrap()
            .listed_pads;
        assert_eq!(pads[0].pad.content, "New");
    }
}
