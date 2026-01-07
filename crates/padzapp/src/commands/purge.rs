use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::index::DisplayIndex;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::DataStore;
use std::io::{self, Write};
use uuid::Uuid;

use super::helpers::{indexed_pads, pads_by_selectors};

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    skip_confirm: bool,
) -> Result<CmdResult> {
    // 1. Resolve targets
    let pads_to_purge = if selectors.is_empty() {
        // If no selectors, target ALL currently deleted pads
        let all_pads = indexed_pads(store, scope)?;
        all_pads
            .into_iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Deleted(_)))
            .collect()
    } else {
        // Specific pads
        pads_by_selectors(store, scope, selectors, true)?
    };

    // 2a. Expand with descendants
    let target_ids: Vec<Uuid> = pads_to_purge.iter().map(|dp| dp.pad.metadata.id).collect();
    let descendants = super::helpers::get_descendant_ids(store, scope, &target_ids)?;

    // Combine unique IDs
    let mut all_ids = target_ids.clone();
    all_ids.extend(descendants.clone());
    // remove duplicates if any (though get_descendant_ids checks children)
    all_ids.sort();
    all_ids.dedup();

    if all_ids.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to purge."));
        return Ok(res);
    }

    // 2. Confirm
    if !skip_confirm {
        println!("This will permanently remove the following pads:");
        for dp in &pads_to_purge {
            println!("  {} {}", dp.index, dp.pad.metadata.title);
        }
        if !descendants.is_empty() {
            println!("  ... and {} descendant(s)", descendants.len());
        }

        print!("[Y] To delete: ");
        io::stdout().flush().map_err(PadzError::Io)?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(PadzError::Io)?;

        if input.trim() != "Y" {
            let mut res = CmdResult::default();
            res.add_message(CmdMessage::info("Operation cancelled."));
            return Ok(res);
        }
    }

    // 3. Delete ALL
    let mut result = CmdResult::default();
    for id in all_ids {
        // Fetch title for message logic? pad might be in pads_to_purge or fetched now.
        // We only reported main targets.
        // Just delete.
        // Check if it exists before trying to delete (might be double deleted if loop logic was loose, but we deduped)
        if store.get_pad(&id, scope).is_ok() {
            store.delete_pad(&id, scope)?;
        }
    }

    // Add success messages for main targets only?
    // Or just summary?
    // User expects feedback.
    for dp in pads_to_purge {
        result.add_message(CmdMessage::success(format!(
            "Purged: {} {}",
            dp.index, dp.pad.metadata.title
        )));
    }
    if !descendants.is_empty() {
        result.add_message(CmdMessage::success(format!(
            "And purged {} descendants",
            descendants.len()
        )));
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
    fn purges_deleted_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Delete it
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
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

        // Purge
        let res = run(
            &mut store,
            Scope::Project,
            &[],
            true, // skip_confirm
        )
        .unwrap();

        assert_eq!(res.messages.len(), 1);
        assert!(res.messages[0].content.contains("Purged: d1 A"));

        // Verify empty
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
    fn purges_specific_pads_even_if_active() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Purge active pad 1
        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            true,
        )
        .unwrap();

        assert_eq!(res.messages.len(), 1);
        assert!(res.messages[0].content.contains("Purged: 1 A"));

        // Verify gone
        let listed = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(listed.listed_pads.len(), 0);
    }

    #[test]
    fn does_nothing_if_no_deleted_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Purge deleted (none)
        let res = run(&mut store, Scope::Project, &[], true).unwrap();

        assert_eq!(res.messages.len(), 1);
        assert!(res.messages[0].content.contains("No pads to purge"));

        // A still exists
        let listed = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
    }

    #[test]
    fn purges_recursively() {
        let mut store = InMemoryStore::new();
        // Create Parent
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        // Create Child inside Parent (id=1)
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Delete Parent
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Verify Child is also effectively hidden/deleted?
        // Wait, DELETE command logic is soft-delete PARENT only.
        // Child is implicitly hidden from Active views.
        // It should appear in Deleted view if parent is indexable.

        let _deleted = get::run(
            &store,
            Scope::Project,
            get::PadFilter {
                status: get::PadStatusFilter::Deleted,
                ..Default::default()
            },
        )
        .unwrap();
        // Should show Parent (d1).
        // Since get command output is recursive, we don't know if child is counted in "listed_pads" (only roots).
        // But purge uses logic to find descendants.

        // Purge Parent
        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Deleted(1)])],
            true,
        )
        .unwrap();

        assert!(res.messages[0].content.contains("Purged: d1 Parent"));
        // Check for descendant message
        let has_descendant_msg = res
            .messages
            .iter()
            .any(|m| m.content.contains("And purged 1 descendants"));
        assert!(has_descendant_msg);

        // Verify Store is empty
        let all_pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(all_pads.len(), 0);
    }
}
