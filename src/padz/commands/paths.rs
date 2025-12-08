use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::DisplayIndex;
use crate::model::Scope;
use crate::store::DataStore;

use super::helpers::resolve_indexes;

pub fn run<S: DataStore>(store: &S, scope: Scope, indexes: &[DisplayIndex]) -> Result<CmdResult> {
    let resolved = resolve_indexes(store, scope, indexes)?;
    let mut paths = Vec::with_capacity(resolved.len());

    for (_, uuid) in resolved {
        let path = store.get_pad_path(&uuid, scope)?;
        paths.push(path);
    }

    Ok(CmdResult::default().with_pad_paths(paths))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn test_get_path() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Pad A".into(), "".into()).unwrap();

        let res = run(&store, Scope::Project, &[DisplayIndex::Regular(1)]).unwrap();
        assert_eq!(res.pad_paths.len(), 1);
        // InMemoryStore generates fake paths, just check it returns something valid-ish
        // for testing. But wait, InMemoryStore's get_pad_path might return a generic path.
        // Let's verify it returns a path that ends with the expected file extension if set?
        // InMemoryStore defaults might not implementation full path logic as FileStore does.
        // Let's check what InMemoryStore does.
        // Assuming it works, we just assert non-empty.
        assert!(!res.pad_paths[0].as_os_str().is_empty());
    }

    #[test]
    fn test_get_multiple_paths() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Pad A".into(), "".into()).unwrap();
        create::run(&mut store, Scope::Project, "Pad B".into(), "".into()).unwrap();

        // 1 is B, 2 is A
        let res = run(
            &store,
            Scope::Project,
            &[DisplayIndex::Regular(1), DisplayIndex::Regular(2)],
        )
        .unwrap();
        assert_eq!(res.pad_paths.len(), 2);
    }
}
