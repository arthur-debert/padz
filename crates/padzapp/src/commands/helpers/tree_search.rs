use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad};
use crate::model::Scope;
use crate::store::DataStore;
use uuid::Uuid;

use super::indexing::indexed_pads;

pub fn get_descendant_ids<S: DataStore>(
    store: &S,
    scope: Scope,
    target_ids: &[Uuid],
) -> Result<Vec<Uuid>> {
    let roots = indexed_pads(store, scope)?;
    let mut seen = std::collections::HashSet::new();
    let mut descendants = Vec::new();

    for target in target_ids {
        if let Some(node) = find_node_by_id(&roots, *target) {
            collect_subtree_ids(node, &mut descendants, &mut seen);
        }
    }
    Ok(descendants)
}

pub(super) fn find_node_by_id(pads: &[DisplayPad], id: Uuid) -> Option<&DisplayPad> {
    for dp in pads {
        if dp.pad.metadata.id == id {
            return Some(dp);
        }
        if let Some(found) = find_node_by_id(&dp.children, id) {
            return Some(found);
        }
    }
    None
}

fn collect_subtree_ids(
    dp: &DisplayPad,
    ids: &mut Vec<Uuid>,
    seen: &mut std::collections::HashSet<Uuid>,
) {
    for child in &dp.children {
        if seen.insert(child.pad.metadata.id) {
            ids.push(child.pad.metadata.id);
        }
        collect_subtree_ids(child, ids, seen);
    }
}

/// Finds a pad in the tree by UUID, optionally filtering by index type.
///
/// The `index_filter` predicate determines which index types are acceptable.
/// Common patterns:
/// - `|_| true` - find any pad with matching UUID
/// - `|idx| matches!(idx, DisplayIndex::Regular(_))` - find restored pad
/// - `|idx| matches!(idx, DisplayIndex::Pinned(_))` - find pinned pad
pub fn find_pad_by_uuid<F>(pads: &[DisplayPad], uuid: Uuid, index_filter: F) -> Option<&DisplayPad>
where
    F: Fn(&DisplayIndex) -> bool + Copy,
{
    for dp in pads {
        if dp.pad.metadata.id == uuid && index_filter(&dp.index) {
            return Some(dp);
        }
        if let Some(found) = find_pad_by_uuid(&dp.children, uuid, index_filter) {
            return Some(found);
        }
    }
    None
}
