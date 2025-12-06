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
