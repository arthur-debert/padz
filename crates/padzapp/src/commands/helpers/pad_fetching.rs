use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;

use super::indexing::bucket_for_index;
use super::selector_resolve::{resolve_selectors, TitleBucket};

/// Resolves selectors and returns their complete display paths with flat pads.
///
/// The path preserves the canonical hierarchical identity selected by the user,
/// while each [`DisplayPad`] keeps the operation-friendly local index and has
/// `children: Vec::new()`.
pub fn pads_with_paths_by_selectors<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    check_delete_protection: bool,
    title_bucket: TitleBucket,
) -> Result<Vec<(Vec<DisplayIndex>, DisplayPad)>> {
    let resolved = resolve_selectors(
        store,
        scope,
        selectors,
        check_delete_protection,
        title_bucket,
    )?;
    let mut pads = Vec::with_capacity(resolved.len());
    for (path, id) in resolved {
        let local_index = path.last().cloned().unwrap_or(DisplayIndex::Regular(0));
        let bucket = bucket_for_index(&local_index);
        let pad = store.get_pad(&id, scope, bucket)?;

        pads.push((
            path,
            DisplayPad {
                pad,
                index: local_index,
                matches: None,
                children: Vec::new(),
            },
        ));
    }
    Ok(pads)
}

/// Resolves selectors and returns a flat list of DisplayPads.
///
/// **Important**: This returns a *flattened* view—each pad has `children: Vec::new()`.
/// The `index` field contains only the *local* index (last path segment), not the full path.
/// Use this for operations that act on individual pads (delete, pin, etc.).
///
/// For hierarchical data, use `indexed_pads()` instead.
pub fn pads_by_selectors<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    check_delete_protection: bool,
    title_bucket: TitleBucket,
) -> Result<Vec<DisplayPad>> {
    Ok(pads_with_paths_by_selectors(
        store,
        scope,
        selectors,
        check_delete_protection,
        title_bucket,
    )?
    .into_iter()
    .map(|(_, pad)| pad)
    .collect())
}
