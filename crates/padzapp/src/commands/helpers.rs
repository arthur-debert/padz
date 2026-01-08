use crate::error::{PadzError, Result};
use crate::index::{index_pads, DisplayIndex, DisplayPad};
use crate::model::Scope;
use crate::store::DataStore;
use uuid::Uuid;

pub fn indexed_pads<S: DataStore>(store: &S, scope: Scope) -> Result<Vec<DisplayPad>> {
    let pads = store.list_pads(scope)?;
    Ok(index_pads(pads))
}

use crate::index::PadSelector;

pub fn resolve_selectors<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    check_delete_protection: bool,
) -> Result<Vec<(Vec<DisplayIndex>, Uuid)>> {
    let root_pads = indexed_pads(store, scope)?;

    // Linearize the tree for range resolution and search
    let linearized = linearize_tree(&root_pads);

    let mut results = Vec::new();

    for selector in selectors {
        match selector {
            PadSelector::Path(path) => {
                if let Some((_, pad)) = find_in_linearized(&linearized, path) {
                    if check_delete_protection && pad.pad.metadata.delete_protected {
                        return Err(PadzError::Api(
                            "Pinned pads are delete protected, unpin then delete it".to_string(),
                        ));
                    }
                    results.push((path.clone(), pad.pad.metadata.id));
                } else {
                    let s: Vec<String> = path.iter().map(|idx| idx.to_string()).collect();
                    return Err(PadzError::Api(format!(
                        "Index {} not found in current scope",
                        s.join(".")
                    )));
                }
            }
            PadSelector::Range(start_path, end_path) => {
                let start_idx = linearized
                    .iter()
                    .position(|(p, _)| p == start_path)
                    .ok_or_else(|| {
                        PadzError::Api(format!("Range start {} not found", fmt_path(start_path)))
                    })?;
                let end_idx = linearized
                    .iter()
                    .position(|(p, _)| p == end_path)
                    .ok_or_else(|| {
                        PadzError::Api(format!("Range end {} not found", fmt_path(end_path)))
                    })?;

                if start_idx > end_idx {
                    return Err(PadzError::Api(format!(
                        "Invalid range: {} appears after {} in the list",
                        fmt_path(start_path),
                        fmt_path(end_path)
                    )));
                }

                // Collect all items in range inclusive
                for (path, pad) in linearized.iter().take(end_idx + 1).skip(start_idx) {
                    if check_delete_protection && pad.pad.metadata.delete_protected {
                        return Err(PadzError::Api(
                            "Pinned pads are delete protected, unpin then delete it".to_string(),
                        ));
                    }
                    results.push((path.clone(), pad.pad.metadata.id));
                }
            }
            PadSelector::Title(term) => {
                let term_lower = term.to_lowercase();
                let matches: Vec<&(Vec<DisplayIndex>, &DisplayPad)> = linearized
                    .iter()
                    .filter(|(_, dp)| {
                        if dp.pad.metadata.title.to_lowercase().contains(&term_lower) {
                            return true;
                        }
                        dp.pad.content.to_lowercase().contains(&term_lower)
                    })
                    .collect();

                match matches.len() {
                    0 => return Err(PadzError::Api(format!("No pad found matching \"{}\"", term))),
                    1 => {
                        let (path, dp) = matches[0];
                        if check_delete_protection && dp.pad.metadata.delete_protected {
                             return Err(PadzError::Api("Pinned pads are delete protected, unpin then delete it".to_string()));
                        }
                        results.push((path.clone(), dp.pad.metadata.id));
                    },
                    n => return Err(PadzError::Api(format!(
                        "Term \"{}\" matches multiple paths, add more to make it unique(matched {} pads). Please be more specific.",
                        term, n
                    ))),
                }
            }
        }
    }

    Ok(results)
}

fn linearize_tree(roots: &[DisplayPad]) -> Vec<(Vec<DisplayIndex>, &DisplayPad)> {
    let mut result = Vec::new();
    for pad in roots {
        linearize_recursive(pad, Vec::new(), &mut result);
    }
    result
}

fn linearize_recursive<'a>(
    pad: &'a DisplayPad,
    parent_path: Vec<DisplayIndex>,
    result: &mut Vec<(Vec<DisplayIndex>, &'a DisplayPad)>,
) {
    let mut current_path = parent_path;
    current_path.push(pad.index.clone());

    result.push((current_path.clone(), pad));

    for child in &pad.children {
        linearize_recursive(child, current_path.clone(), result);
    }
}

fn find_in_linearized<'a>(
    linearized: &'a [(Vec<DisplayIndex>, &'a DisplayPad)],
    path: &[DisplayIndex],
) -> Option<&'a (Vec<DisplayIndex>, &'a DisplayPad)> {
    linearized.iter().find(|(p, _)| p == path)
}

pub fn fmt_path(path: &[DisplayIndex]) -> String {
    let s: Vec<String> = path.iter().map(|idx| idx.to_string()).collect();
    s.join(".")
}

pub fn pads_by_selectors<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    check_delete_protection: bool,
) -> Result<Vec<DisplayPad>> {
    let resolved = resolve_selectors(store, scope, selectors, check_delete_protection)?;
    let mut pads = Vec::with_capacity(resolved.len());
    for (path, id) in resolved {
        let pad = store.get_pad(&id, scope)?;
        // We need search matches logic for DisplayPad if we want it?
        // But we are constructing it from scratch here without children.
        // It's flattened.
        // And the index is just the last segment (local), but typically DisplayPad stores local index.
        // If we want FULL path info in DisplayPad... index.rs struct has local index.
        // We can reconstruct tree or just return simple items.
        // The callers of this usually operate on items.
        // However, DisplayPad requires children field now.

        let local_index = path.last().cloned().unwrap_or(DisplayIndex::Regular(0)); // Should not be empty

        pads.push(DisplayPad {
            pad,
            index: local_index,
            matches: None,
            children: Vec::new(), // Flattened view
        });
    }
    Ok(pads)
}

pub fn get_descendant_ids<S: DataStore>(
    store: &S,
    scope: Scope,
    target_ids: &[Uuid],
) -> Result<Vec<Uuid>> {
    let all_pads = store.list_pads(scope)?;
    let roots = index_pads(all_pads);
    let mut descendants = Vec::new();

    for target in target_ids {
        if let Some(node) = find_node_by_id(&roots, *target) {
            collect_subtree_ids(node, &mut descendants);
        }
    }
    Ok(descendants)
}

fn find_node_by_id(pads: &[DisplayPad], id: Uuid) -> Option<&DisplayPad> {
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

fn collect_subtree_ids(dp: &DisplayPad, ids: &mut Vec<Uuid>) {
    for child in &dp.children {
        ids.push(child.pad.metadata.id);
        collect_subtree_ids(child, ids);
    }
}

/// Finds a pad in the tree by UUID, optionally filtering by index type.
///
/// The `index_filter` predicate determines which index types are acceptable.
/// Common patterns:
/// - `|_| true` - find any pad with matching UUID
/// - `|idx| matches!(idx, DisplayIndex::Regular(_))` - find restored pad
/// - `|idx| matches!(idx, DisplayIndex::Pinned(_))` - find pinned pad
pub fn find_pad_by_uuid<F>(
    pads: &[DisplayPad],
    uuid: Uuid,
    index_filter: F,
) -> Option<&DisplayPad>
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
