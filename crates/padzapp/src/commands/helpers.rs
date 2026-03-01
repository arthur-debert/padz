use crate::error::{PadzError, Result};
use crate::index::{index_pads, DisplayIndex, DisplayPad};
use crate::model::Scope;
use crate::store::Bucket;
use crate::store::DataStore;
use uuid::Uuid;

pub fn indexed_pads<S: DataStore>(store: &S, scope: Scope) -> Result<Vec<DisplayPad>> {
    let active_pads = store.list_pads(scope, Bucket::Active)?;
    let archived_pads = store.list_pads(scope, Bucket::Archived)?;
    let deleted_pads = store.list_pads(scope, Bucket::Deleted)?;

    Ok(index_pads(active_pads, archived_pads, deleted_pads))
}

use crate::index::PadSelector;

pub fn resolve_selectors<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    check_delete_protection: bool,
) -> Result<Vec<(Vec<DisplayIndex>, Uuid)>> {
    let root_pads = indexed_pads(store, scope)?;

    // Linearize the tree for range resolution and search
    let linearized = linearize_tree(&root_pads);

    let mut results = Vec::new();

    for selector in selectors {
        match selector {
            PadSelector::Path(path) => {
                if let Some((_, pad)) = find_in_linearized(&linearized, path) {
                    if check_delete_protection && pad.pad.metadata.delete_protected {
                        return Err(PadzError::Api(
                            "Pinned pads are delete protected, unpin then delete it".to_string(),
                        ));
                    }
                    results.push((path.clone(), pad.pad.metadata.id));
                } else {
                    let s: Vec<String> = path.iter().map(|idx| idx.to_string()).collect();
                    return Err(PadzError::Api(format!(
                        "Index {} not found in current scope",
                        s.join(".")
                    )));
                }
            }
            PadSelector::Range(start_path, end_path) => {
                let start_idx = linearized
                    .iter()
                    .position(|(p, _)| p == start_path)
                    .ok_or_else(|| {
                        PadzError::Api(format!("Range start {} not found", fmt_path(start_path)))
                    })?;
                let end_idx = linearized
                    .iter()
                    .position(|(p, _)| p == end_path)
                    .ok_or_else(|| {
                        PadzError::Api(format!("Range end {} not found", fmt_path(end_path)))
                    })?;

                if start_idx > end_idx {
                    return Err(PadzError::Api(format!(
                        "Invalid range: {} appears after {} in the list",
                        fmt_path(start_path),
                        fmt_path(end_path)
                    )));
                }

                // Collect all items in range inclusive
                for (path, pad) in linearized.iter().take(end_idx + 1).skip(start_idx) {
                    if check_delete_protection && pad.pad.metadata.delete_protected {
                        return Err(PadzError::Api(
                            "Pinned pads are delete protected, unpin then delete it".to_string(),
                        ));
                    }
                    results.push((path.clone(), pad.pad.metadata.id));
                }
            }
            PadSelector::Uuid(uuid) => {
                // Search the entire linearized tree for a pad matching this UUID
                let found = linearized
                    .iter()
                    .find(|(_, dp)| dp.pad.metadata.id == *uuid);

                match found {
                    Some((path, dp)) => {
                        if check_delete_protection && dp.pad.metadata.delete_protected {
                            return Err(PadzError::Api(
                                "Pinned pads are delete protected, unpin then delete it"
                                    .to_string(),
                            ));
                        }
                        results.push((path.clone(), dp.pad.metadata.id));
                    }
                    None => {
                        return Err(PadzError::Api(format!("No pad found with UUID {}", uuid)));
                    }
                }
            }
            PadSelector::Title(term) => {
                let term_lower = term.to_lowercase();
                let matches: Vec<&(Vec<DisplayIndex>, &DisplayPad)> = linearized
                    .iter()
                    .filter(|(_, dp)| dp.pad.metadata.title.to_lowercase().contains(&term_lower))
                    .collect();

                match matches.len() {
                    0 => return Err(PadzError::Api(format!("No pad found matching \"{}\"", term))),
                    1 => {
                        let (path, dp) = matches[0];
                        if check_delete_protection && dp.pad.metadata.delete_protected {
                             return Err(PadzError::Api("Pinned pads are delete protected, unpin then delete it".to_string()));
                        }
                        results.push((path.clone(), dp.pad.metadata.id));
                    },
                    n => return Err(PadzError::Api(format!(
                        "Term \"{}\" matches multiple paths, add more to make it unique(matched {} pads). Please be more specific.",
                        term, n
                    ))),
                }
            }
        }
    }

    Ok(results)
}

fn linearize_tree(roots: &[DisplayPad]) -> Vec<(Vec<DisplayIndex>, &DisplayPad)> {
    let mut result = Vec::new();
    for pad in roots {
        linearize_recursive(pad, Vec::new(), &mut result);
    }
    result
}

fn linearize_recursive<'a>(
    pad: &'a DisplayPad,
    parent_path: Vec<DisplayIndex>,
    result: &mut Vec<(Vec<DisplayIndex>, &'a DisplayPad)>,
) {
    let mut current_path = parent_path;
    current_path.push(pad.index.clone());

    result.push((current_path.clone(), pad));

    for child in &pad.children {
        linearize_recursive(child, current_path.clone(), result);
    }
}

fn find_in_linearized<'a>(
    linearized: &'a [(Vec<DisplayIndex>, &'a DisplayPad)],
    path: &[DisplayIndex],
) -> Option<&'a (Vec<DisplayIndex>, &'a DisplayPad)> {
    linearized.iter().find(|(p, _)| p == path)
}

pub fn fmt_path(path: &[DisplayIndex]) -> String {
    let s: Vec<String> = path.iter().map(|idx| idx.to_string()).collect();
    s.join(".")
}

/// Determines the storage bucket for a pad based on its display index.
pub fn bucket_for_index(index: &DisplayIndex) -> Bucket {
    match index {
        DisplayIndex::Pinned(_) | DisplayIndex::Regular(_) => Bucket::Active,
        DisplayIndex::Archived(_) => Bucket::Archived,
        DisplayIndex::Deleted(_) => Bucket::Deleted,
    }
}

/// Resolves selectors and returns a flat list of DisplayPads.
///
/// **Important**: This returns a *flattened* view—each pad has `children: Vec::new()`.
/// The `index` field contains only the *local* index (last path segment), not the full path.
/// Use this for operations that act on individual pads (delete, pin, etc.).
///
/// For hierarchical data, use `indexed_pads()` instead.
pub fn pads_by_selectors<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    check_delete_protection: bool,
) -> Result<Vec<DisplayPad>> {
    let resolved = resolve_selectors(store, scope, selectors, check_delete_protection)?;
    let mut pads = Vec::with_capacity(resolved.len());
    for (path, id) in resolved {
        let local_index = path.last().cloned().unwrap_or(DisplayIndex::Regular(0));
        let bucket = bucket_for_index(&local_index);
        let pad = store.get_pad(&id, scope, bucket)?;

        pads.push(DisplayPad {
            pad,
            index: local_index,
            matches: None,
            children: Vec::new(),
        });
    }
    Ok(pads)
}

pub fn get_descendant_ids<S: DataStore>(
    store: &S,
    scope: Scope,
    target_ids: &[Uuid],
) -> Result<Vec<Uuid>> {
    let roots = indexed_pads(store, scope)?;
    let mut seen = std::collections::HashSet::new();
    let mut descendants = Vec::new();

    for target in target_ids {
        if let Some(node) = find_node_by_id(&roots, *target) {
            collect_subtree_ids(node, &mut descendants, &mut seen);
        }
    }
    Ok(descendants)
}

fn find_node_by_id(pads: &[DisplayPad], id: Uuid) -> Option<&DisplayPad> {
    for dp in pads {
        if dp.pad.metadata.id == id {
            return Some(dp);
        }
        if let Some(found) = find_node_by_id(&dp.children, id) {
            return Some(found);
        }
    }
    None
}

fn collect_subtree_ids(
    dp: &DisplayPad,
    ids: &mut Vec<Uuid>,
    seen: &mut std::collections::HashSet<Uuid>,
) {
    for child in &dp.children {
        if seen.insert(child.pad.metadata.id) {
            ids.push(child.pad.metadata.id);
        }
        collect_subtree_ids(child, ids, seen);
    }
}

/// Finds a pad in the tree by UUID, optionally filtering by index type.
///
/// The `index_filter` predicate determines which index types are acceptable.
/// Common patterns:
/// - `|_| true` - find any pad with matching UUID
/// - `|idx| matches!(idx, DisplayIndex::Regular(_))` - find restored pad
/// - `|idx| matches!(idx, DisplayIndex::Pinned(_))` - find pinned pad
pub fn find_pad_by_uuid<F>(pads: &[DisplayPad], uuid: Uuid, index_filter: F) -> Option<&DisplayPad>
where
    F: Fn(&DisplayIndex) -> bool + Copy,
{
    for dp in pads {
        if dp.pad.metadata.id == uuid && index_filter(&dp.index) {
            return Some(dp);
        }
        if let Some(found) = find_pad_by_uuid(&dp.children, uuid, index_filter) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn test_range_selection_within_siblings() {
        // Create parent with 3 children, test range 1.1-1.3
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
            "Child A".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child B".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child C".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Select range 1.1-1.3 (all children)
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1), DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(1), DisplayIndex::Regular(3)],
            )],
            false,
        )
        .unwrap();

        // Should get 3 children (newest first: C=1.1, B=1.2, A=1.3)
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_range_selection_cross_parent() {
        // Create 2 parents with children, test range 1.1-2.1
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Parent 1 with child
        create::run(
            &mut store,
            Scope::Project,
            "Parent 1".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child 1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Parent 2 with child
        create::run(
            &mut store,
            Scope::Project,
            "Parent 2".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child 2".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Note: After creation, order is (newest first):
        // 1: Parent 2, 1.1: Child 2
        // 2: Parent 1, 2.1: Child 1

        // Select range 1.1-2.1 should linearize to: 1, 1.1, 2, 2.1
        // and select from 1.1 to 2.1: [1.1, 2, 2.1]
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1), DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(2), DisplayIndex::Regular(1)],
            )],
            false,
        )
        .unwrap();

        // Should include: Child 2 (1.1), Parent 1 (2), Child 1 (2.1)
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_range_selection_root_only() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create: Root1 -> Child1, Root2
        create::run(&mut store, Scope::Project, "Root 1".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child 1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(&mut store, Scope::Project, "Root 2".into(), "".into(), None).unwrap();

        // Order (newest first): Root 2 (1), Root 1 (2) with Child 1 (2.1)
        // Linear order: 1, 2, 2.1
        // Range 1-2 selects from index 1 to 2, NOT including 2.1 (comes after 2 in linear order)
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(2)],
            )],
            false,
        )
        .unwrap();

        // Should get Root 2 (1) and Root 1 (2) only, NOT Child 1 (2.1)
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_range_includes_children_of_intermediate_nodes() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create: Root1 -> Child1, Root2, Root3
        create::run(&mut store, Scope::Project, "Root 1".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child 1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(&mut store, Scope::Project, "Root 2".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Root 3".into(), "".into(), None).unwrap();

        // Order (newest first): Root 3 (1), Root 2 (2), Root 1 (3) with Child 1 (3.1)
        // Linear order: 1, 2, 3, 3.1
        // Range 1-3.1 selects all
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(3), DisplayIndex::Regular(1)],
            )],
            false,
        )
        .unwrap();

        // Should get all 4: Root 3, Root 2, Root 1, Child 1
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_pinned_child_addressable_by_path() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create parent with pinned child
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Pin the child
        crate::commands::pinning::pin(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        )
        .unwrap();

        // Should be addressable as 1.p1
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Pinned(1),
            ])],
            false,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        // Verify it's the child
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "Child");
    }

    #[test]
    fn test_title_search_no_match_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Alpha".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Beta".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Gamma".to_string())],
            false,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("No pad found matching"));
        assert!(err.to_string().contains("Gamma"));
    }

    #[test]
    fn test_title_search_multiple_matches_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Meeting Monday".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Meeting Tuesday".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Meeting Wednesday".into(),
            "".into(),
            None,
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Meeting".to_string())],
            false,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("multiple"));
        assert!(err.to_string().contains("3")); // matched 3 pads
    }

    #[test]
    fn test_title_search_single_match_succeeds() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Alpha".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Beta".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Gamma".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Beta".to_string())],
            false,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "Beta");
    }

    #[test]
    fn test_title_search_matches_title_only_not_content() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Shopping List".into(),
            "Buy apples and oranges".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Todo List".into(),
            "Call dentist".into(),
            None,
        )
        .unwrap();

        // Search for content that's in body, not title - should NOT match
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("apples".to_string())],
            false,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No pad found"));

        // Search for title should work
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Shopping".to_string())],
            false,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "Shopping List");
    }

    #[test]
    fn test_title_search_is_case_insensitive() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "UPPERCASE TITLE".into(),
            "".into(),
            None,
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("uppercase".to_string())],
            false,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "UPPERCASE TITLE");
    }

    #[test]
    fn test_title_search_delete_protection_check() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "ProtectedPad".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Manually set delete_protected without pinning (to avoid dual-index)
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads[0].clone();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        // Try to resolve with delete protection check enabled
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("ProtectedPad".to_string())],
            true, // check_delete_protection = true
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }

    #[test]
    fn test_title_search_delete_protection_disabled() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "ProtectedPad".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Manually set delete_protected without pinning (to avoid dual-index)
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads[0].clone();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        // Resolve with delete protection check disabled
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("ProtectedPad".to_string())],
            false, // check_delete_protection = false
        )
        .unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_uuid_resolution() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result =
            create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        let pad_uuid = result.affected_pads[0].pad.metadata.id;

        let resolved = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Uuid(pad_uuid)],
            false,
        )
        .unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].1, pad_uuid);
    }

    #[test]
    fn test_uuid_not_found_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        let fake_uuid = uuid::Uuid::new_v4();
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Uuid(fake_uuid)],
            false,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("No pad found with UUID"));
    }

    #[test]
    fn test_uuid_delete_protection() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result =
            create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        let pad_uuid = result.affected_pads[0].pad.metadata.id;

        // Set delete protection
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads[0].clone();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Uuid(pad_uuid)],
            true, // check_delete_protection
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }

    #[test]
    fn test_range_invalid_order_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create pads: newest first, so order is 1, 2, 3
        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad B".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad C".into(), "".into(), None).unwrap();

        // Try to select range 3-1 (reversed order)
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(3)],
                vec![DisplayIndex::Regular(1)],
            )],
            false,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("appears after"));
    }

    #[test]
    fn test_range_start_not_found_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        // Try to select range starting from nonexistent index
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(99)],
                vec![DisplayIndex::Regular(1)],
            )],
            false,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Range start"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_range_end_not_found_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        // Try to select range ending at nonexistent index
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(99)],
            )],
            false,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Range end"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_range_delete_protection_check() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad B".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad C".into(), "".into(), None).unwrap();

        // Protect the middle pad (index 2)
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad_b = pads
            .iter()
            .find(|p| p.metadata.title == "Pad B")
            .unwrap()
            .clone();
        pad_b.metadata.delete_protected = true;
        store
            .save_pad(&pad_b, Scope::Project, Bucket::Active)
            .unwrap();

        // Try to select range 1-3 with delete protection check
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(3)],
            )],
            true, // check_delete_protection = true
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }

    #[test]
    fn test_path_selector_not_found_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(99)])],
            false,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_path_selector_delete_protection_check() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        // Protect the pad
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads[0].clone();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            true, // check_delete_protection = true
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }
}
