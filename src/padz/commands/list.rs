use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::DisplayIndex;
use crate::model::Scope;
use crate::store::DataStore;

use super::helpers::indexed_pads;

pub fn run<S: DataStore>(store: &S, scope: Scope, show_deleted: bool) -> Result<CmdResult> {
    let pads = indexed_pads(store, scope)?;
    let listed: Vec<_> = if show_deleted {
        pads.into_iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Deleted(_)))
            .collect()
    } else {
        pads.into_iter()
            .filter(|dp| !matches!(dp.index, DisplayIndex::Deleted(_)))
            .collect()
    };

    Ok(CmdResult::default().with_listed_pads(listed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, delete};
    use crate::index::DisplayIndex;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn lists_active_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into()).unwrap();

        let result = run(&store, Scope::Project, false).unwrap();
        assert_eq!(result.listed_pads.len(), 1);
        assert!(matches!(
            result.listed_pads[0].index,
            DisplayIndex::Regular(1)
        ));
    }

    #[test]
    fn lists_deleted_only_when_requested() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into()).unwrap();
        let idx = DisplayIndex::Regular(1);
        delete::run(&mut store, Scope::Project, &[idx]).unwrap();

        let result = run(&store, Scope::Project, true).unwrap();
        assert_eq!(result.listed_pads.len(), 1);
        assert!(matches!(
            result.listed_pads[0].index,
            DisplayIndex::Deleted(1)
        ));
    }
}
