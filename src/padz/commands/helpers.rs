use crate::error::{PadzError, Result};
use crate::index::{index_pads, DisplayIndex, DisplayPad};
use crate::model::Scope;
use crate::store::DataStore;
use uuid::Uuid;

pub fn indexed_pads<S: DataStore>(store: &S, scope: Scope) -> Result<Vec<DisplayPad>> {
    let pads = store.list_pads(scope)?;
    Ok(index_pads(pads))
}

pub fn resolve_indexes<S: DataStore>(
    store: &S,
    scope: Scope,
    indexes: &[DisplayIndex],
) -> Result<Vec<(DisplayIndex, Uuid)>> {
    let indexed = indexed_pads(store, scope)?;

    indexes
        .iter()
        .map(|idx| {
            indexed
                .iter()
                .find(|dp| &dp.index == idx)
                .map(|dp| (idx.clone(), dp.pad.metadata.id))
                .ok_or_else(|| PadzError::Api(format!("Index {} not found in current scope", idx)))
        })
        .collect()
}

pub fn pads_by_indexes<S: DataStore>(
    store: &S,
    scope: Scope,
    indexes: &[DisplayIndex],
) -> Result<Vec<DisplayPad>> {
    let resolved = resolve_indexes(store, scope, indexes)?;
    let mut pads = Vec::with_capacity(resolved.len());
    for (index, id) in resolved {
        let pad = store.get_pad(&id, scope)?;
        pads.push(DisplayPad { pad, index });
    }
    Ok(pads)
}
