use crate::index::{DisplayIndex, DisplayPad};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadStatusFilter {
    All,
    Active,
    Archived,
    Deleted,
    Pinned,
}

/// Recursively filters the tree based on status.
///
/// Filtering rules:
/// - **Active**: Show non-deleted pads. Children are recursively filtered (only non-deleted).
/// - **Deleted**: Show deleted pads with ALL their children (children aren't marked deleted
///   but are visible under their deleted parent per spec: "unless looking at deleted items").
/// - **Pinned**: Show pinned pads. Children are recursively filtered for pinned only.
/// - **All**: Show everything, no filtering.
pub(super) fn filter_tree(pads: Vec<DisplayPad>, status: PadStatusFilter) -> Vec<DisplayPad> {
    pads.into_iter()
        .filter_map(|dp| filter_pad_recursive(dp, status))
        .collect()
}

fn filter_pad_recursive(mut dp: DisplayPad, status: PadStatusFilter) -> Option<DisplayPad> {
    let dominated = matches_status(&dp.index, status);

    if !dominated {
        return None;
    }

    // For Deleted status, show ALL children (they inherit visibility from deleted parent)
    // For other statuses, recursively filter children
    if status == PadStatusFilter::Deleted {
        dp.children = dp
            .children
            .into_iter()
            .map(filter_children_under_deleted)
            .collect();
    } else {
        dp.children = dp
            .children
            .into_iter()
            .filter_map(|child| filter_pad_recursive(child, status))
            .collect();
    }

    Some(dp)
}

/// When viewing deleted pads, children of a deleted parent are shown.
/// Those children might have their own children that need filtering.
fn filter_children_under_deleted(mut dp: DisplayPad) -> DisplayPad {
    dp.children = dp
        .children
        .into_iter()
        .map(filter_children_under_deleted)
        .collect();
    dp
}

fn matches_status(index: &DisplayIndex, status: PadStatusFilter) -> bool {
    match status {
        PadStatusFilter::All => true,
        PadStatusFilter::Active => {
            matches!(index, DisplayIndex::Pinned(_) | DisplayIndex::Regular(_))
        }
        PadStatusFilter::Archived => matches!(index, DisplayIndex::Archived(_)),
        PadStatusFilter::Deleted => matches!(index, DisplayIndex::Deleted(_)),
        PadStatusFilter::Pinned => matches!(index, DisplayIndex::Pinned(_)),
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::get::{run, PadFilter, PadStatusFilter};
    use crate::commands::{create, delete};
    use crate::index::{DisplayIndex, PadSelector};
    use crate::model::Scope;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn test_active_filter_shows_nested_children() {
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

        let res = run(&store, Scope::Project, PadFilter::default(), &[]).unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Parent");
        assert_eq!(res.listed_pads[0].children.len(), 1);
        assert_eq!(res.listed_pads[0].children[0].pad.metadata.title, "Child");
    }

    #[test]
    fn test_active_filter_hides_deleted_child() {
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
            "Child1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child2".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Delete Child1 (newest child = 1.1)
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        )
        .unwrap();

        let res = run(&store, Scope::Project, PadFilter::default(), &[]).unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].children.len(), 1);
        assert_eq!(res.listed_pads[0].children[0].pad.metadata.title, "Child1");
    }

    #[test]
    fn test_deleted_filter_shows_parent_with_children() {
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

        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Deleted,
                search_term: None,
                todo_status: None,
                tags: None,
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Parent");
        assert_eq!(res.listed_pads[0].children.len(), 1);
        assert_eq!(res.listed_pads[0].children[0].pad.metadata.title, "Child");
    }

    #[test]
    fn test_active_filter_hides_children_of_deleted_parent() {
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

        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        let res = run(&store, Scope::Project, PadFilter::default(), &[]).unwrap();

        assert_eq!(res.listed_pads.len(), 0);
    }
}
