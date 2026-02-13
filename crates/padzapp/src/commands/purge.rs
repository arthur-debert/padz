use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, PadSelector};
use crate::model::{Scope, TodoStatus};
use crate::store::DataStore;
use uuid::Uuid;

use super::helpers::{indexed_pads, pads_by_selectors};

/// Permanently removes pads from the store.
///
/// **Confirmation required**: The `confirmed` parameter must be `true` to proceed.
/// If `false`, returns an error instructing the user to use `--yes` or `-y`.
///
/// **Safety valve**: When purging pads that have children, the `recursive` flag must be set.
/// This prevents accidental deletion of entire subtrees.
///
/// - If `selectors` is empty, targets all deleted pads (plus Done pads if `include_done` is true)
/// - If `recursive` is false and any target has children, returns an error
/// - If `confirmed` is false, returns an error (no pads are deleted)
/// - `include_done`: when true and no selectors given, also purges pads with Done status
pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    recursive: bool,
    confirmed: bool,
    include_done: bool,
) -> Result<CmdResult> {
    // 1. Resolve targets
    let pads_to_purge = if selectors.is_empty() {
        let all_pads = indexed_pads(store, scope)?;
        all_pads
            .into_iter()
            .filter(|dp| {
                matches!(dp.index, DisplayIndex::Deleted(_))
                    || (include_done && dp.pad.metadata.status == TodoStatus::Done)
            })
            .collect()
    } else {
        pads_by_selectors(store, scope, selectors, true)?
    };

    if pads_to_purge.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to purge."));
        return Ok(res);
    }

    // 2. Find descendants
    let target_ids: Vec<Uuid> = pads_to_purge.iter().map(|dp| dp.pad.metadata.id).collect();
    let descendants = super::helpers::get_descendant_ids(store, scope, &target_ids)?;

    // 3. Safety valve: require --recursive if there are children
    if !descendants.is_empty() && !recursive {
        return Err(PadzError::Api(format!(
            "Cannot purge: {} pad(s) have children. Use --recursive (-r) to purge entire subtrees.",
            pads_to_purge
                .iter()
                .filter(|dp| {
                    let id = dp.pad.metadata.id;
                    super::helpers::get_descendant_ids(store, scope, &[id])
                        .map(|d| !d.is_empty())
                        .unwrap_or(false)
                })
                .count()
        )));
    }

    // 4. Calculate total count for message
    let total_count = pads_to_purge.len() + descendants.len();

    // 5. Confirmation check - must come after we know the count
    if !confirmed {
        return Err(PadzError::Api(format!(
            "Purging {} pad(s). Aborted, confirm with --yes or -y for hard deletion.",
            total_count
        )));
    }

    // 6. Execute the purge
    let mut all_ids = target_ids;
    all_ids.extend(descendants.clone());
    all_ids.sort();
    all_ids.dedup();

    let mut result = CmdResult::default();

    // Add the "Purging X padz..." message first
    result.add_message(CmdMessage::info(format!(
        "Purging {} pad(s)...",
        total_count
    )));

    for id in all_ids {
        if store.get_pad(&id, scope).is_ok() {
            store.delete_pad(&id, scope)?;
        }
    }

    for dp in pads_to_purge {
        result.add_message(CmdMessage::success(format!(
            "Purged: {} {}",
            dp.index, dp.pad.metadata.title
        )));
    }
    if !descendants.is_empty() {
        result.add_message(CmdMessage::success(format!(
            "And purged {} descendant(s)",
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
    fn purges_deleted_pads_when_confirmed() {
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
            &[],
        )
        .unwrap();
        assert_eq!(deleted.listed_pads.len(), 1);

        // Purge with confirmed=true
        let res = run(
            &mut store,
            Scope::Project,
            &[],
            false, // recursive not needed
            true,  // confirmed
            false, // include_done
        )
        .unwrap();

        // Should have "Purging 1 pad(s)..." and "Purged: d1 A"
        assert!(res.messages.iter().any(|m| m.content.contains("Purging 1")));
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Purged: d1 A")));

        // Verify empty
        let deleted_after = get::run(
            &store,
            Scope::Project,
            get::PadFilter {
                status: get::PadStatusFilter::Deleted,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert_eq!(deleted_after.listed_pads.len(), 0);
    }

    #[test]
    fn purge_without_confirmation_returns_error() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Delete it
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Purge with confirmed=false - should fail
        let result = run(
            &mut store,
            Scope::Project,
            &[],
            false, // recursive
            false, // confirmed = false
            false, // include_done
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Aborted"));
        assert!(err.to_string().contains("--yes"));
        assert!(err.to_string().contains("-y"));

        // Verify pad is still there (not purged)
        let deleted = get::run(
            &store,
            Scope::Project,
            get::PadFilter {
                status: get::PadStatusFilter::Deleted,
                ..Default::default()
            },
            &[],
        )
        .unwrap();
        assert_eq!(deleted.listed_pads.len(), 1);
    }

    #[test]
    fn purges_specific_pads_even_if_active() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Purge active pad 1 (no children, so recursive not needed)
        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            false, // recursive
            true,  // confirmed
            false, // include_done
        )
        .unwrap();

        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Purged: 1 A")));

        // Verify gone
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 0);
    }

    #[test]
    fn does_nothing_if_no_deleted_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Purge deleted (none) - even with confirmed=true, should just say "No pads"
        let res = run(&mut store, Scope::Project, &[], false, true, false).unwrap();

        assert_eq!(res.messages.len(), 1);
        assert!(res.messages[0].content.contains("No pads to purge"));

        // A still exists
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
    }

    #[test]
    fn purges_recursively_with_flag() {
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

        // Purge Parent WITH recursive flag and confirmed
        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Deleted(1)])],
            true,  // recursive = true
            true,  // confirmed = true
            false, // include_done
        )
        .unwrap();

        assert!(res.messages.iter().any(|m| m.content.contains("Purging 2"))); // parent + child
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("Purged: d1 Parent")));
        assert!(res
            .messages
            .iter()
            .any(|m| m.content.contains("And purged 1 descendant")));

        // Verify Store is empty
        let all_pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(all_pads.len(), 0);
    }

    #[test]
    fn purge_without_recursive_fails_when_has_children() {
        let mut store = InMemoryStore::new();
        // Create Parent
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        // Create Child inside Parent
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Try to purge Parent WITHOUT recursive flag - should fail
        let result = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            false, // recursive = false
            true,  // confirmed = true
            false, // include_done
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("have children"));
        assert!(err.to_string().contains("--recursive"));

        // Verify nothing was deleted
        let all_pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(all_pads.len(), 2);
    }

    #[test]
    fn purge_selectors_vs_all() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();

        // Delete both to make them purgeable candidates
        delete::run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
            ],
        )
        .unwrap();

        // Purge only one (selectors provided)
        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Deleted(1)])],
            false, // recursive
            true,  // confirmed
            false, // include_done
        )
        .unwrap();

        assert!(res.messages.iter().any(|m| m.content.contains("Purging 1")));

        let remaining = store.list_pads(Scope::Project).unwrap();
        assert_eq!(remaining.len(), 1); // One remains
        assert!(remaining[0].metadata.is_deleted);
    }

    #[test]
    fn purge_nothing_found() {
        let mut store = InMemoryStore::new();
        // Empty store - even with confirmed=true
        let res = run(&mut store, Scope::Project, &[], false, true, false).unwrap();
        assert!(res.messages[0].content.contains("No pads to purge"));
    }

    #[test]
    fn purge_error_includes_count() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();

        // Delete both
        delete::run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
            ],
        )
        .unwrap();

        // Purge without confirmation - error should show count
        let result = run(&mut store, Scope::Project, &[], false, false, false);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Purging 2 pad(s)"));
    }

    #[test]
    fn purge_include_done_removes_completed_pads() {
        let mut store = InMemoryStore::new();
        create::run(
            &mut store,
            Scope::Project,
            "Keep Me".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Complete Me".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Newest-first: "Complete Me" = index 1, "Keep Me" = index 2
        // Complete "Complete Me" (index 1)
        crate::commands::status::complete(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Purge with include_done=true (no selectors)
        let res = run(&mut store, Scope::Project, &[], false, true, true).unwrap();

        // Should purge the Done pad
        assert!(res.messages.iter().any(|m| m.content.contains("Purging 1")));

        // "Keep Me" (Planned) should still exist
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
        assert_eq!(listed.listed_pads[0].pad.metadata.title, "Keep Me");
    }

    #[test]
    fn purge_include_done_false_ignores_completed_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Complete pad A
        crate::commands::status::complete(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // Purge with include_done=false (default / notes mode)
        let res = run(&mut store, Scope::Project, &[], false, true, false).unwrap();

        // No pads to purge (Done but not Deleted, and include_done is false)
        assert!(res.messages[0].content.contains("No pads to purge"));

        // A still exists
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
    }

    #[test]
    fn purge_include_done_and_deleted_together() {
        let mut store = InMemoryStore::new();
        // Created in order: oldest first. Newest-first indexing means:
        // "Active Pad" = 1 (newest), "Deleted Pad" = 2, "Done Pad" = 3 (oldest)
        create::run(
            &mut store,
            Scope::Project,
            "Done Pad".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Deleted Pad".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Active Pad".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Complete "Active Pad" (index 1, newest) - wait, we want to keep it.
        // Complete "Done Pad" (index 3)
        crate::commands::status::complete(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(3)])],
        )
        .unwrap();

        // Delete "Deleted Pad" (index 2)
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(2)])],
        )
        .unwrap();

        // Purge with include_done=true - should get both Done and Deleted
        let res = run(&mut store, Scope::Project, &[], false, true, true).unwrap();

        assert!(res.messages.iter().any(|m| m.content.contains("Purging 2")));

        // Only "Active Pad" should remain
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
        assert_eq!(listed.listed_pads[0].pad.metadata.title, "Active Pad");
    }
}
