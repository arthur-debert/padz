use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad};
use crate::model::Scope;
use crate::store::DataStore;

use super::indexing::indexed_pads;
use super::tree_search::find_node_by_id;

/// A pad with its nesting depth, produced by tree-walking.
#[derive(Debug, Clone)]
pub struct NestedPad {
    pub pad: DisplayPad,
    pub depth: usize,
}

/// Given a list of resolved (flat) DisplayPads, re-resolve them with their full
/// subtrees from the indexed tree. Returns a flat sequence of (pad, depth) pairs
/// in tree-traversal order.
///
/// Each selected pad is at depth 0, its children at depth 1, etc.
/// Only active (non-deleted) children are included.
pub fn collect_nested_pads<S: DataStore>(
    store: &S,
    scope: Scope,
    root_pads: &[DisplayPad],
) -> Result<Vec<NestedPad>> {
    let indexed = indexed_pads(store, scope)?;
    let mut result = Vec::new();

    for dp in root_pads {
        if let Some(tree_node) = find_node_by_id(&indexed, dp.pad.metadata.id) {
            flatten_tree(tree_node, 0, &mut result);
        } else {
            result.push(NestedPad {
                pad: dp.clone(),
                depth: 0,
            });
        }
    }

    Ok(result)
}

fn flatten_tree(dp: &DisplayPad, depth: usize, result: &mut Vec<NestedPad>) {
    result.push(NestedPad {
        pad: DisplayPad {
            pad: dp.pad.clone(),
            index: dp.index.clone(),
            matches: dp.matches.clone(),
            children: Vec::new(),
        },
        depth,
    });
    for child in &dp.children {
        if !matches!(child.index, DisplayIndex::Deleted(_)) {
            flatten_tree(child, depth + 1, result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::commands::helpers::pads_by_selectors;
    use crate::index::{DisplayIndex, PadSelector};
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn collect_nested_returns_parent_then_children_with_depths() {
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

        let flat = pads_by_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            false,
        )
        .unwrap();
        assert_eq!(flat.len(), 1);

        let nested = collect_nested_pads(&store, Scope::Project, &flat).unwrap();

        assert_eq!(nested.len(), 3);
        assert_eq!(nested[0].pad.pad.metadata.title, "Parent");
        assert_eq!(nested[0].depth, 0);
        assert_eq!(nested[1].depth, 1);
        assert_eq!(nested[2].depth, 1);
    }

    #[test]
    fn collect_nested_deep_tree_tracks_depth() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Root".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Level 1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Level 2".into(),
            "".into(),
            Some(PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])),
        )
        .unwrap();

        let flat = pads_by_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            false,
        )
        .unwrap();
        let nested = collect_nested_pads(&store, Scope::Project, &flat).unwrap();

        let depths: Vec<usize> = nested.iter().map(|np| np.depth).collect();
        assert_eq!(depths, vec![0, 1, 2]);
    }

    #[test]
    fn collect_nested_skips_deleted_children() {
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
            "Keep".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Delete Me".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        crate::commands::delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        )
        .unwrap();

        let flat = pads_by_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            false,
        )
        .unwrap();
        let nested = collect_nested_pads(&store, Scope::Project, &flat).unwrap();

        assert_eq!(nested.len(), 2);
        assert_eq!(nested[0].pad.pad.metadata.title, "Parent");
        assert_eq!(nested[1].pad.pad.metadata.title, "Keep");
    }

    #[test]
    fn collect_nested_leaf_pad_returns_single() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Leaf".into(),
            "body".into(),
            None,
        )
        .unwrap();

        let flat = pads_by_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            false,
        )
        .unwrap();
        let nested = collect_nested_pads(&store, Scope::Project, &flat).unwrap();

        assert_eq!(nested.len(), 1);
        assert_eq!(nested[0].depth, 0);
        assert_eq!(nested[0].pad.pad.metadata.title, "Leaf");
    }

    #[test]
    fn collect_nested_multiple_roots_each_expand() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Root A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Root B".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child of A".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(2)])),
        )
        .unwrap();

        let flat = pads_by_selectors(
            &store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
            ],
            false,
        )
        .unwrap();
        let nested = collect_nested_pads(&store, Scope::Project, &flat).unwrap();

        assert_eq!(nested.len(), 3);
        assert_eq!(nested[0].pad.pad.metadata.title, "Root B");
        assert_eq!(nested[0].depth, 0);
        assert_eq!(nested[1].pad.pad.metadata.title, "Root A");
        assert_eq!(nested[1].depth, 0);
        assert_eq!(nested[2].pad.pad.metadata.title, "Child of A");
        assert_eq!(nested[2].depth, 1);
    }
}
