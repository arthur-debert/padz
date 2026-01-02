use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::index::DisplayIndex;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::DataStore;
use std::io::{self, Write};

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

    if pads_to_purge.is_empty() {
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

    // 3. Delete
    let mut result = CmdResult::default();
    for dp in pads_to_purge {
        store.delete_pad(&dp.pad.metadata.id, scope)?;
        result.add_message(CmdMessage::success(format!(
            "Purged: {} {}",
            dp.index, dp.pad.metadata.title
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
        create::run(&mut store, Scope::Project, "A".into(), "".into()).unwrap();

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
        create::run(&mut store, Scope::Project, "A".into(), "".into()).unwrap();

        // Purge active pad 1
        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Index(DisplayIndex::Regular(1))],
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
        create::run(&mut store, Scope::Project, "A".into(), "".into()).unwrap();

        // Purge deleted (none)
        let res = run(&mut store, Scope::Project, &[], true).unwrap();

        assert_eq!(res.messages.len(), 1);
        assert!(res.messages[0].content.contains("No pads to purge"));

        // A still exists
        let listed = get::run(&store, Scope::Project, get::PadFilter::default()).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
    }
}
