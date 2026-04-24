use crate::attributes::AttrFilter;
use crate::index::DisplayPad;

/// Recursively filters the tree based on attribute filters.
/// Returns pads that match ALL specified filters (AND logic), preserving hierarchy.
pub(super) fn apply_attr_filters(pads: Vec<DisplayPad>, filters: &[AttrFilter]) -> Vec<DisplayPad> {
    if filters.is_empty() {
        return pads;
    }
    pads.into_iter()
        .filter_map(|dp| filter_pad_by_attrs(dp, filters))
        .collect()
}

fn filter_pad_by_attrs(mut dp: DisplayPad, filters: &[AttrFilter]) -> Option<DisplayPad> {
    dp.children = dp
        .children
        .into_iter()
        .filter_map(|child| filter_pad_by_attrs(child, filters))
        .collect();

    let matches_all = filters.iter().all(|f| f.matches(&dp.pad.metadata));
    if matches_all || !dp.children.is_empty() {
        Some(dp)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::commands::get::{run, PadFilter, PadStatusFilter};
    use crate::commands::{create, tagging, tags};
    use crate::index::{DisplayIndex, PadSelector};
    use crate::model::{Scope, TodoStatus};
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;
    use crate::store::{Bucket, DataStore};

    // --- TodoStatus filtering tests ---

    #[test]
    fn test_todo_status_filter_planned() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Planned1".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Planned2".into(),
            "".into(),
            None,
        )
        .unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads
            .iter()
            .find(|p| p.metadata.title == "Planned1")
            .unwrap()
            .clone();
        pad.metadata.status = TodoStatus::Done;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::Planned),
                tags: None,
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Planned2");
    }

    #[test]
    fn test_todo_status_filter_done() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Todo1".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Todo2".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Todo3".into(), "".into(), None).unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        for title in ["Todo1", "Todo2"] {
            let mut pad = pads
                .iter()
                .find(|p| p.metadata.title == title)
                .unwrap()
                .clone();
            pad.metadata.status = TodoStatus::Done;
            store
                .save_pad(&pad, Scope::Project, Bucket::Active)
                .unwrap();
        }

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::Done),
                tags: None,
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);
        let titles: Vec<_> = res
            .listed_pads
            .iter()
            .map(|dp| dp.pad.metadata.title.as_str())
            .collect();
        assert!(titles.contains(&"Todo1"));
        assert!(titles.contains(&"Todo2"));
    }

    #[test]
    fn test_todo_status_filter_in_progress() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Task1".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Task2".into(), "".into(), None).unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads
            .iter()
            .find(|p| p.metadata.title == "Task1")
            .unwrap()
            .clone();
        pad.metadata.status = TodoStatus::InProgress;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::InProgress),
                tags: None,
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Task1");
    }

    #[test]
    fn test_todo_status_filter_none_shows_all() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Planned".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(&mut store, Scope::Project, "Done".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "InProgress".into(),
            "".into(),
            None,
        )
        .unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();

        let mut done_pad = pads
            .iter()
            .find(|p| p.metadata.title == "Done")
            .unwrap()
            .clone();
        done_pad.metadata.status = TodoStatus::Done;
        store
            .save_pad(&done_pad, Scope::Project, Bucket::Active)
            .unwrap();

        let mut ip_pad = pads
            .iter()
            .find(|p| p.metadata.title == "InProgress")
            .unwrap()
            .clone();
        ip_pad.metadata.status = TodoStatus::InProgress;
        store
            .save_pad(&ip_pad, Scope::Project, Bucket::Active)
            .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: None,
                tags: None,
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 3);
    }

    #[test]
    fn test_todo_status_filter_preserves_index() {
        // Per spec: "Statuses do not alter the canonical display index"
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "First".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Second".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Third".into(), "".into(), None).unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads
            .iter()
            .find(|p| p.metadata.title == "Second")
            .unwrap()
            .clone();
        pad.metadata.status = TodoStatus::Done;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::Planned),
                tags: None,
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);

        let third = res
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "Third")
            .unwrap();
        assert!(matches!(third.index, DisplayIndex::Regular(1)));

        let first = res
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "First")
            .unwrap();
        assert!(matches!(first.index, DisplayIndex::Regular(3)));
    }

    #[test]
    fn test_todo_status_filter_with_nested_pads() {
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

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut child1 = pads
            .iter()
            .find(|p| p.metadata.title == "Child1")
            .unwrap()
            .clone();
        child1.metadata.status = TodoStatus::Done;
        store
            .save_pad(&child1, Scope::Project, Bucket::Active)
            .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::Planned),
                tags: None,
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Parent");

        assert_eq!(res.listed_pads[0].children.len(), 1);
        assert_eq!(res.listed_pads[0].children[0].pad.metadata.title, "Child2");
    }

    // --- Tag filtering tests ---

    #[test]
    fn test_tag_filter_single_tag() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        tags::create_tag(&mut store, Scope::Project, "work").unwrap();
        tags::create_tag(&mut store, Scope::Project, "rust").unwrap();

        create::run(&mut store, Scope::Project, "Pad1".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad2".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad3".into(), "".into(), None).unwrap();

        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(3)])],
            &["work".to_string()],
        )
        .unwrap();

        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(2)])],
            &["rust".to_string()],
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: None,
                tags: Some(vec!["work".to_string()]),
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Pad1");
    }

    #[test]
    fn test_tag_filter_multiple_tags_and_logic() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        tags::create_tag(&mut store, Scope::Project, "work").unwrap();
        tags::create_tag(&mut store, Scope::Project, "rust").unwrap();

        create::run(&mut store, Scope::Project, "Pad1".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad2".into(), "".into(), None).unwrap();

        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(2)])],
            &["work".to_string(), "rust".to_string()],
        )
        .unwrap();

        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            &["work".to_string()],
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: None,
                tags: Some(vec!["work".to_string(), "rust".to_string()]),
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Pad1");
    }

    #[test]
    fn test_tag_filter_no_matches() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        tags::create_tag(&mut store, Scope::Project, "work").unwrap();

        create::run(&mut store, Scope::Project, "Pad1".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: None,
                tags: Some(vec!["work".to_string()]),
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 0);
    }

    #[test]
    fn test_tag_filter_empty_tags_shows_all() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Pad1".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad2".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: None,
                tags: Some(vec![]),
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);
    }

    #[test]
    fn test_tag_filter_preserves_index() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        tags::create_tag(&mut store, Scope::Project, "work").unwrap();

        create::run(&mut store, Scope::Project, "First".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Second".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Third".into(), "".into(), None).unwrap();

        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(3)])],
            &["work".to_string()],
        )
        .unwrap();
        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            &["work".to_string()],
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: None,
                tags: Some(vec!["work".to_string()]),
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);

        let third = res
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "Third")
            .unwrap();
        assert!(matches!(third.index, DisplayIndex::Regular(1)));

        let first = res
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "First")
            .unwrap();
        assert!(matches!(first.index, DisplayIndex::Regular(3)));
    }

    #[test]
    fn test_tag_filter_with_nested_pads() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        tags::create_tag(&mut store, Scope::Project, "work").unwrap();

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

        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            &["work".to_string()],
        )
        .unwrap();
        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(2),
            ])],
            &["work".to_string()],
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: None,
                tags: Some(vec!["work".to_string()]),
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Parent");

        assert_eq!(res.listed_pads[0].children.len(), 1);
        assert_eq!(res.listed_pads[0].children[0].pad.metadata.title, "Child1");
    }

    #[test]
    fn test_tag_filter_combined_with_search() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        tags::create_tag(&mut store, Scope::Project, "work").unwrap();

        create::run(
            &mut store,
            Scope::Project,
            "Rust Guide".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Python Guide".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Rust Notes".into(),
            "".into(),
            None,
        )
        .unwrap();

        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(3)])],
            &["work".to_string()],
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: Some("rust".into()),
                todo_status: None,
                tags: Some(vec!["work".to_string()]),
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Rust Guide");
    }
}
