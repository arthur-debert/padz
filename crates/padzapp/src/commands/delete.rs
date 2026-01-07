use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;

use super::helpers::resolve_selectors;

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    let resolved = resolve_selectors(store, scope, selectors, true)?;
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
    use crate::commands::{create, get};
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn marks_pad_as_deleted() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into()).unwrap();
        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Index(DisplayIndex::Regular(1))],
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
        create::run(&mut store, Scope::Project, "Protected".into(), "".into()).unwrap();

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
            &[PadSelector::Index(DisplayIndex::Regular(1))],
        );

        assert!(result.is_err());
        match result {
            Err(crate::error::PadzError::Api(msg)) => {
                assert!(msg.contains("Pinned pads are delete protected"));
            }
            _ => panic!("Expected Api error"),
        }
    }
}
