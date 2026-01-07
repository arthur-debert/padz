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
    // Resetting loop logic entirely
    // The first `success_count` was unused due to shadowing. Removed it.

    use uuid::Uuid;

    // Check compliance: If any descendant is pinned/protected, we abort.
    let target_ids: Vec<Uuid> = resolved.iter().map(|(_, id)| *id).collect();
    let descendants = super::helpers::get_descendant_ids(store, scope, &target_ids)?;

    // Check descendants for protection
    for descendant_id in descendants {
        let p = store.get_pad(&descendant_id, scope)?;
        if p.metadata.delete_protected {
            // Find pretty name for error? "pX.X" isn't easy to construct without index.
            // Just use Title.
            return Err(crate::error::PadzError::Api(format!(
                "Cannot delete because descendant '{}' is pinned/protected",
                p.metadata.title
            )));
        }
    }

    for (path, id) in resolved {
        let s: Vec<String> = path.iter().map(|idx| idx.to_string()).collect();
        let display = s.join(".");

        // Fetch pad first to return it
        let pad = match store.get_pad(&id, scope) {
            Ok(pad) => pad,
            Err(e) => {
                result.add_message(CmdMessage::error(format!(
                    "Failed to find pad {}: {}",
                    display, e
                )));
                continue;
            }
        };

        // Soft delete
        let mut pad_to_update = pad.clone();
        pad_to_update.metadata.is_deleted = true;
        pad_to_update.metadata.deleted_at = Some(Utc::now());

        match store.save_pad(&pad_to_update, scope) {
            Ok(_) => {
                result.add_message(CmdMessage::success(format!(
                    "Deleted pad {}: {}",
                    display, pad.metadata.title
                )));
                result.affected_pads.push(pad);
            }
            Err(e) => {
                result.add_message(CmdMessage::error(format!(
                    "Failed to delete pad {}: {}",
                    display, e
                )));
            }
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
}
