use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::{index_pads, DisplayPad};
use crate::model::Scope;
use crate::store::DataStore;

pub fn run<S: DataStore>(store: &S, scope: Scope, term: &str) -> Result<CmdResult> {
    let pads = store.list_pads(scope)?;
    let indexed = index_pads(pads);
    let term_lower = term.to_lowercase();

    let mut matches: Vec<(DisplayPad, u8)> = indexed
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

    let listed = matches.into_iter().map(|(dp, _)| dp).collect();
    Ok(CmdResult::default().with_listed_pads(listed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn ranks_exact_title_matches_first() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Foo Bar".into(), "".into()).unwrap();
        create::run(&mut store, Scope::Project, "Bar".into(), "".into()).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Another".into(),
            "Bar content".into(),
        )
        .unwrap();

        let result = run(&store, Scope::Project, "Bar").unwrap();
        assert_eq!(result.listed_pads.len(), 3);
        assert_eq!(result.listed_pads[0].pad.metadata.title, "Bar");
        assert_eq!(result.listed_pads[1].pad.metadata.title, "Foo Bar");
    }
}
