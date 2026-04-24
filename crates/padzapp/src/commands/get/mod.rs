use crate::attributes::{AttrFilter, AttrValue};
use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::{DisplayPad, PadSelector};
use crate::model::{Scope, TodoStatus};
use crate::store::DataStore;

mod attr_filter;
mod search;
mod selector_filter;
mod status_filter;

pub use status_filter::PadStatusFilter;

#[derive(Debug, Clone)]
pub struct PadFilter {
    pub status: PadStatusFilter,
    pub search_term: Option<String>,
    /// Filter by todo status. None means show all (no filtering by todo status).
    pub todo_status: Option<TodoStatus>,
    /// Filter by tags. None means show all (no filtering by tags).
    /// Multiple tags means AND logic - pads must have ALL specified tags.
    pub tags: Option<Vec<String>>,
}

impl Default for PadFilter {
    fn default() -> Self {
        Self {
            status: PadStatusFilter::Active,
            search_term: None,
            todo_status: None,
            tags: None,
        }
    }
}

pub fn run<S: DataStore>(
    store: &S,
    scope: Scope,
    filter: PadFilter,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    let indexed = super::helpers::indexed_pads(store, scope)?;

    // 0. Filter by ID selectors (if any)
    let indexed = if selectors.is_empty() {
        indexed
    } else {
        selector_filter::filter_by_selectors(indexed, selectors)?
    };

    // 1. Filter by deletion status (Active/Deleted/Pinned)
    let mut filtered: Vec<DisplayPad> = status_filter::filter_tree(indexed, filter.status);

    // 2. Build attribute filters from filter options
    let mut attr_filters: Vec<AttrFilter> = Vec::new();

    if let Some(todo_status) = filter.todo_status {
        let status_str = format!("{:?}", todo_status);
        attr_filters.push(AttrFilter::eq("status", AttrValue::Enum(status_str)));
    }

    if let Some(ref tags) = filter.tags {
        if !tags.is_empty() {
            attr_filters.push(AttrFilter::contains_all("tags", tags.clone()));
        }
    }

    // 3. Apply unified attribute filters
    filtered = attr_filter::apply_attr_filters(filtered, &attr_filters);

    // 4. Apply search if needed
    if let Some(term) = &filter.search_term {
        filtered = search::apply_search(filtered, term);
    }

    Ok(CmdResult::default().with_listed_pads(filtered))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, delete};
    use crate::index::DisplayIndex;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn test_filters() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Active".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Deleted".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Delete the second one
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // 1. Test Active
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
        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Active");

        // 2. Test Deleted
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
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Deleted");

        // 3. Test All
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::All,
                search_term: None,
                todo_status: None,
                tags: None,
            },
            &[],
        )
        .unwrap();
        assert_eq!(res.listed_pads.len(), 2);
    }

    #[test]
    fn test_search() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Foo".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Bar".into(),
            "contains foo".into(),
            None,
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: Some("foo".into()),
                todo_status: None,
                tags: None,
            },
            &[],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);
        // "Foo" title match (score 10) > "Bar" content match (score 5)
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Foo");

        let matches_0 = res.listed_pads[0].matches.as_ref().unwrap();
        assert!(matches_0.iter().any(|m| m.line_number == 0)); // Title match

        let matches_1 = res.listed_pads[1].matches.as_ref().unwrap();
        assert!(matches_1.iter().any(|m| m.line_number == 3)); // Content match
    }
}
