use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::{index_pads, DisplayIndex, DisplayPad};
use crate::model::Scope;
use crate::store::DataStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadStatusFilter {
    All,
    Active,
    Deleted,
    Pinned, // Only pinned? Or pinned active? Let's say pinned active.
}

#[derive(Debug, Clone)]
pub struct PadFilter {
    pub status: PadStatusFilter,
    pub search_term: Option<String>,
}

impl Default for PadFilter {
    fn default() -> Self {
        Self {
            status: PadStatusFilter::Active,
            search_term: None,
        }
    }
}

pub fn run<S: DataStore>(store: &S, scope: Scope, filter: PadFilter) -> Result<CmdResult> {
    // 1. Fetch relevant pads based on status to minimize processing
    // Currently store only has list_pads returning all.
    // If we want to optimize store access we would need store changes.
    // For now we fetch all and filter in memory as current implementation does.
    let pads = store.list_pads(scope)?;
    let indexed = index_pads(pads);

    let mut filtered: Vec<DisplayPad> = indexed
        .into_iter()
        .filter(|dp| match filter.status {
            PadStatusFilter::All => true,
            PadStatusFilter::Active => !matches!(dp.index, DisplayIndex::Deleted(_)),
            PadStatusFilter::Deleted => matches!(dp.index, DisplayIndex::Deleted(_)),
            PadStatusFilter::Pinned => matches!(dp.index, DisplayIndex::Pinned(_)),
        })
        .collect();

    // 2. Apply search if needed
    if let Some(term) = &filter.search_term {
        let term_lower = term.to_lowercase();
        let mut matches: Vec<(DisplayPad, u8)> = filtered
            .into_iter()
            .filter_map(|dp| {
                let title_lower = dp.pad.metadata.title.to_lowercase();
                let content_lower = dp.pad.content.to_lowercase();

                let score = if title_lower == term_lower {
                    1
                } else if title_lower.contains(&term_lower) {
                    2
                } else if content_lower.contains(&term_lower) {
                    3
                } else {
                    return None;
                };

                Some((dp, score))
            })
            .collect();

        // Sort by score then metadata
        matches.sort_by(|(a, score_a), (b, score_b)| match score_a.cmp(score_b) {
            std::cmp::Ordering::Equal => {
                let len_a = a.pad.metadata.title.len();
                let len_b = b.pad.metadata.title.len();
                match len_a.cmp(&len_b) {
                    std::cmp::Ordering::Equal => {
                        a.pad.metadata.created_at.cmp(&b.pad.metadata.created_at)
                    }
                    ord => ord,
                }
            }
            ord => ord,
        });

        filtered = matches.into_iter().map(|(dp, _)| dp).collect();
    }

    Ok(CmdResult::default().with_listed_pads(filtered))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, delete};
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn test_filters() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Active".into(), "".into()).unwrap();
        create::run(&mut store, Scope::Project, "Deleted".into(), "".into()).unwrap();

        // Delete the second one
        // "Deleted" should be index 1 (newest) at this point before deletion?
        // Wait, creating Active then Deleted. Deleted is newest (1). Active is 2.
        // Delete 1.
        delete::run(&mut store, Scope::Project, &[DisplayIndex::Regular(1)]).unwrap();

        // 1. Test Active
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
            },
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
            },
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
            },
        )
        .unwrap();
        assert_eq!(res.listed_pads.len(), 2);
    }

    #[test]
    fn test_search() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Foo".into(), "".into()).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Bar".into(),
            "contains foo".into(),
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: Some("foo".into()),
            },
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);
        // "Foo" title match (score 1) > "Bar" content match (score 3)
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Foo");
    }
}
