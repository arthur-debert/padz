use crate::error::Result;
use crate::index::{current_ordering_key, index_pads, DisplayIndex, DisplayPad};
use crate::model::Scope;
use crate::store::{Bucket, DataStore};

pub fn indexed_pads<S: DataStore>(store: &S, scope: Scope) -> Result<Vec<DisplayPad>> {
    let active_pads = store.list_pads(scope, Bucket::Active)?;
    let archived_pads = store.list_pads(scope, Bucket::Archived)?;
    let deleted_pads = store.list_pads(scope, Bucket::Deleted)?;

    Ok(index_pads(
        active_pads,
        archived_pads,
        deleted_pads,
        current_ordering_key(),
    ))
}

/// Determines the storage bucket for a pad based on its display index.
pub fn bucket_for_index(index: &DisplayIndex) -> Bucket {
    match index {
        DisplayIndex::Pinned(_) | DisplayIndex::Regular(_) => Bucket::Active,
        DisplayIndex::Archived(_) => Bucket::Archived,
        DisplayIndex::Deleted(_) => Bucket::Deleted,
    }
}
