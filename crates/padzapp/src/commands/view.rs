use crate::commands::{CmdResult, NestingMode};
use crate::error::Result;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::{Bucket, DataStore};

use super::helpers::{collect_nested_pads, pads_by_selectors, NestedPad};

pub fn run<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    nesting: NestingMode,
) -> Result<CmdResult> {
    let pads = pads_by_selectors(store, scope, selectors, false)?;

    let nested = match nesting {
        NestingMode::Flat => pads
            .iter()
            .map(|dp| NestedPad {
                pad: dp.clone(),
                depth: 0,
            })
            .collect(),
        NestingMode::Tree | NestingMode::Indented => collect_nested_pads(store, scope, &pads)?,
    };

    // Collect paths for each pad (for editor integration)
    let paths: Vec<_> = nested
        .iter()
        .filter_map(|np| {
            store
                .get_pad_path(&np.pad.pad.metadata.id, scope, Bucket::Active)
                .ok()
        })
        .collect();

    let depths: Vec<usize> = nested.iter().map(|np| np.depth).collect();
    let listed: Vec<_> = nested.into_iter().map(|np| np.pad).collect();
    let mut result = CmdResult::default().with_listed_pads(listed);
    result.listed_depths = depths;
    result.pad_paths = paths;
    result.nesting = nesting;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::index::{DisplayIndex, PadSelector};
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    fn make_store() -> BucketedStore<MemBackend> {
        BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        )
    }

    #[test]
    fn flat_mode_returns_only_selected_pad() {
        let mut store = make_store();
        // Create parent with children
        create::run(
            &mut store,
            Scope::Project,
            "Parent".into(),
            "Parent body".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child A".into(),
            "Child A body".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child B".into(),
            "Child B body".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let result = run(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            NestingMode::Flat,
        )
        .unwrap();

        assert_eq!(result.listed_pads.len(), 1);
        assert_eq!(result.listed_pads[0].pad.metadata.title, "Parent");
        assert_eq!(result.listed_depths, vec![0]);
    }

    #[test]
    fn tree_mode_includes_children_recursively() {
        let mut store = make_store();
        create::run(
            &mut store,
            Scope::Project,
            "Parent".into(),
            "Parent body".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child A".into(),
            "Child A body".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child B".into(),
            "Child B body".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let result = run(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            NestingMode::Tree,
        )
        .unwrap();

        // Should have parent + 2 children = 3 pads
        assert_eq!(result.listed_pads.len(), 3);
        assert_eq!(result.listed_pads[0].pad.metadata.title, "Parent");
        // Children are newest first: Child B (1.1), Child A (1.2)
        assert_eq!(result.listed_pads[1].pad.metadata.title, "Child B");
        assert_eq!(result.listed_pads[2].pad.metadata.title, "Child A");
        assert_eq!(result.listed_depths, vec![0, 1, 1]);
    }

    #[test]
    fn indented_mode_tracks_depth() {
        let mut store = make_store();
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

        let result = run(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            NestingMode::Indented,
        )
        .unwrap();

        assert_eq!(result.listed_pads.len(), 3);
        assert_eq!(result.listed_depths, vec![0, 1, 2]);
        assert_eq!(result.listed_pads[0].pad.metadata.title, "Root");
        assert_eq!(result.listed_pads[1].pad.metadata.title, "Level 1");
        assert_eq!(result.listed_pads[2].pad.metadata.title, "Level 2");
    }

    #[test]
    fn tree_mode_skips_deleted_children() {
        let mut store = make_store();
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Active Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        // Create a child then delete it
        create::run(
            &mut store,
            Scope::Project,
            "Will Delete".into(),
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

        let result = run(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            NestingMode::Tree,
        )
        .unwrap();

        // Parent + 1 active child (deleted child excluded)
        assert_eq!(result.listed_pads.len(), 2);
        assert_eq!(result.listed_pads[0].pad.metadata.title, "Parent");
        assert_eq!(result.listed_pads[1].pad.metadata.title, "Active Child");
    }

    #[test]
    fn tree_mode_on_leaf_pad_returns_just_that_pad() {
        let mut store = make_store();
        create::run(
            &mut store,
            Scope::Project,
            "Leaf".into(),
            "content".into(),
            None,
        )
        .unwrap();

        let result = run(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            NestingMode::Tree,
        )
        .unwrap();

        assert_eq!(result.listed_pads.len(), 1);
        assert_eq!(result.listed_pads[0].pad.metadata.title, "Leaf");
        assert_eq!(result.listed_depths, vec![0]);
    }

    #[test]
    fn tree_mode_multiple_roots_each_expand() {
        let mut store = make_store();
        // Create two parents, each with a child
        create::run(
            &mut store,
            Scope::Project,
            "Parent A".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Parent B".into(),
            "".into(),
            None,
        )
        .unwrap();
        // Parent B is 1, Parent A is 2 (newest first)
        create::run(
            &mut store,
            Scope::Project,
            "Child of A".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(2)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child of B".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let result = run(
            &store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
            ],
            NestingMode::Tree,
        )
        .unwrap();

        // Parent B + Child of B + Parent A + Child of A = 4
        assert_eq!(result.listed_pads.len(), 4);
        assert_eq!(result.listed_pads[0].pad.metadata.title, "Parent B");
        assert_eq!(result.listed_pads[1].pad.metadata.title, "Child of B");
        assert_eq!(result.listed_pads[2].pad.metadata.title, "Parent A");
        assert_eq!(result.listed_pads[3].pad.metadata.title, "Child of A");
        assert_eq!(result.listed_depths, vec![0, 1, 0, 1]);
    }
}
