use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::{Scope, TodoStatus};
use crate::store::Bucket;
use crate::store::DataStore;
use std::collections::HashSet;
use uuid::Uuid;

use super::helpers::{indexed_pads, pads_with_paths_by_selectors, TitleBucket};

/// One explicitly selected pad with its complete canonical display path.
#[derive(Debug, Clone)]
pub struct PurgeSelection {
    pub path: Vec<DisplayIndex>,
    pub pad: DisplayPad,
}

impl PurgeSelection {
    /// Return the complete canonical display selector for this selection.
    pub fn selector(&self) -> String {
        self.path
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(".")
    }
}

/// Semantic result of a permanent-deletion request.
#[derive(Debug, Clone)]
pub enum PurgeOutcome {
    Empty,
    Purged {
        /// Explicit selections, de-duplicated by UUID in display order.
        selected_pads: Vec<PurgeSelection>,
        /// Total number of unique pads permanently deleted.
        total_purged: usize,
        /// Unique deleted descendants that were not explicitly selected.
        descendant_count: usize,
    },
}

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
/// - Duplicate display indexes and selected/descendant overlap count each UUID once
pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    recursive: bool,
    confirmed: bool,
    include_done: bool,
) -> Result<PurgeOutcome> {
    // 1. Resolve targets
    let mut pads_to_purge: Vec<PurgeSelection> = if selectors.is_empty() {
        let all_pads = indexed_pads(store, scope)?;
        all_pads
            .into_iter()
            .filter(|dp| {
                matches!(dp.index, DisplayIndex::Deleted(_))
                    || (include_done && dp.pad.metadata.status == TodoStatus::Done)
            })
            .map(|pad| PurgeSelection {
                path: vec![pad.index.clone()],
                pad,
            })
            .collect()
    } else {
        pads_with_paths_by_selectors(store, scope, selectors, true, TitleBucket::Deleted)?
            .into_iter()
            .map(|(path, pad)| PurgeSelection { path, pad })
            .collect()
    };

    // Pinned active pads appear under both pinned and regular display indexes.
    // Preserve the first display identity while keeping semantic selections unique.
    let mut seen_targets = HashSet::new();
    pads_to_purge.retain(|selection| seen_targets.insert(selection.pad.pad.metadata.id));

    if pads_to_purge.is_empty() {
        return Ok(PurgeOutcome::Empty);
    }

    // 2. Find descendants
    let target_ids: Vec<Uuid> = pads_to_purge
        .iter()
        .map(|selection| selection.pad.pad.metadata.id)
        .collect();
    let descendants = super::helpers::get_descendant_ids(store, scope, &target_ids)?;

    // 3. Safety valve: require --recursive if there are children
    if !descendants.is_empty() && !recursive {
        return Err(PadzError::Api(format!(
            "Cannot purge: {} pad(s) have children. Use --recursive (-r) to purge entire subtrees.",
            pads_to_purge
                .iter()
                .filter(|selection| {
                    let id = selection.pad.pad.metadata.id;
                    super::helpers::get_descendant_ids(store, scope, &[id])
                        .map(|d| !d.is_empty())
                        .unwrap_or(false)
                })
                .count()
        )));
    }

    // 4. Build the unique deletion set. A selected child can also be returned as
    // a descendant of another selected pad, but remains an explicit selection.
    let mut all_ids = target_ids;
    all_ids.extend(descendants);
    all_ids.sort();
    all_ids.dedup();

    let total_count = all_ids.len();
    let descendant_count = total_count - pads_to_purge.len();

    // 5. Confirmation check - must come after we know the count
    if !confirmed {
        return Err(PadzError::Api(format!(
            "Purging {} pad(s). Aborted, confirm with --yes or -y for hard deletion.",
            total_count
        )));
    }

    // 6. Execute the purge
    for id in all_ids {
        // Try each bucket (deleted first, then active for Done pads, then archived)
        for &bucket in &[Bucket::Deleted, Bucket::Active, Bucket::Archived] {
            if store.get_pad(&id, scope, bucket).is_ok() {
                store.delete_pad(&id, scope, bucket)?;
                break;
            }
        }
    }

    Ok(PurgeOutcome::Purged {
        selected_pads: pads_to_purge,
        total_purged: total_count,
        descendant_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, delete, get, pinning};
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    fn assert_purged(
        outcome: PurgeOutcome,
        selected: &[&str],
        total_purged: usize,
        descendant_count: usize,
    ) {
        let PurgeOutcome::Purged {
            selected_pads,
            total_purged: actual_total,
            descendant_count: actual_descendants,
        } = outcome
        else {
            panic!("expected PurgeOutcome::Purged");
        };
        let actual_selected: Vec<String> = selected_pads
            .iter()
            .map(|selection| {
                format!(
                    "{} {}",
                    selection.selector(),
                    selection.pad.pad.metadata.title
                )
            })
            .collect();
        assert_eq!(actual_selected, selected);
        assert_eq!(actual_total, total_purged);
        assert_eq!(actual_descendants, descendant_count);
    }

    #[test]
    fn purges_deleted_pads_when_confirmed() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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

        assert_purged(res, &["d1 A"], 1, 0);

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
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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

        assert_purged(res, &["1 A"], 1, 0);

        // Verify gone
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 0);
    }

    #[test]
    fn does_nothing_if_no_deleted_pads() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();

        // Purge deleted (none) - even with confirmed=true, should just say "No pads"
        let res = run(&mut store, Scope::Project, &[], false, true, false).unwrap();

        assert!(matches!(res, PurgeOutcome::Empty));

        // A still exists
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
    }

    #[test]
    fn purges_recursively_with_flag() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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

        assert_purged(res, &["d1 Parent"], 2, 1);

        // Verify Store is empty (both Active and Deleted)
        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Active)
                .unwrap()
                .len(),
            0
        );
        assert_eq!(
            store
                .list_pads(Scope::Project, Bucket::Deleted)
                .unwrap()
                .len(),
            0
        );
    }

    #[test]
    fn selected_child_is_not_counted_again_as_a_descendant() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(1), DisplayIndex::Regular(1)]),
            ],
            true,
            true,
            false,
        )
        .unwrap();

        assert_purged(res, &["1 Parent", "1.1 Child"], 2, 0);
    }

    #[test]
    fn purge_without_recursive_fails_when_has_children() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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
        let all_pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(all_pads.len(), 2);
    }

    #[test]
    fn purge_selectors_vs_all() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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

        assert_purged(res, &["d1 B"], 1, 0);

        let remaining = store.list_pads(Scope::Project, Bucket::Deleted).unwrap();
        assert_eq!(remaining.len(), 1); // One remains in Deleted bucket
    }

    #[test]
    fn purge_nothing_found() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        // Empty store - even with confirmed=true
        let res = run(&mut store, Scope::Project, &[], false, true, false).unwrap();
        assert!(matches!(res, PurgeOutcome::Empty));
    }

    #[test]
    fn purge_error_includes_count() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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

        assert_purged(res, &["1 Complete Me"], 1, 0);

        // "Keep Me" (Planned) should still exist
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
        assert_eq!(listed.listed_pads[0].pad.metadata.title, "Keep Me");
    }

    #[test]
    fn purge_include_done_reports_a_pinned_pad_once() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Complete Me".into(),
            "".into(),
            None,
        )
        .unwrap();
        crate::commands::status::complete(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();
        pinning::pin(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        let res = run(&mut store, Scope::Project, &[], false, true, true).unwrap();

        assert_purged(res, &["p1 Complete Me"], 1, 0);
    }

    #[test]
    fn purge_include_done_false_ignores_completed_pads() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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

        assert!(matches!(res, PurgeOutcome::Empty));

        // A still exists
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
    }

    #[test]
    fn purge_include_done_and_deleted_together() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
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

        assert_purged(res, &["2 Done Pad", "d1 Deleted Pad"], 2, 0);

        // Only "Active Pad" should remain
        let listed = get::run(&store, Scope::Project, get::PadFilter::default(), &[]).unwrap();
        assert_eq!(listed.listed_pads.len(), 1);
        assert_eq!(listed.listed_pads[0].pad.metadata.title, "Active Pad");
    }
}
