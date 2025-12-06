use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;

use super::helpers::resolve_indexes;

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    indexes: &[crate::index::DisplayIndex],
) -> Result<CmdResult> {
    let resolved = resolve_indexes(store, scope, indexes)?;
    let mut result = CmdResult::default();

    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope)?;
        pad.metadata.is_deleted = true;
        pad.metadata.deleted_at = Some(Utc::now());
        store.save_pad(&pad, scope)?;
        result.add_message(CmdMessage::success(format!(
            "Pad deleted ({}): {}",
            display_index, pad.metadata.title
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

    #[test]
    fn marks_pad_as_deleted() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into()).unwrap();
        run(&mut store, Scope::Project, &[DisplayIndex::Regular(1)]).unwrap();

        let deleted = list::run(&store, Scope::Project, true).unwrap();
        assert_eq!(deleted.listed_pads.len(), 1);
        assert!(matches!(
            deleted.listed_pads[0].index,
            DisplayIndex::Deleted(1)
        ));
    }
}
