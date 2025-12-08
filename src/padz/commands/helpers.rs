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
) -> Result<Vec<(DisplayIndex, Uuid)>> {
    let indexed = indexed_pads(store, scope)?;

    selectors
        .iter()
        .map(|selector| match selector {
            PadSelector::Index(idx) => indexed
                .iter()
                .find(|dp| &dp.index == idx)
                .map(|dp| (idx.clone(), dp.pad.metadata.id))
                .ok_or_else(|| PadzError::Api(format!("Index {} not found in current scope", idx))),
            PadSelector::Title(term) => {
                let term_lower = term.to_lowercase();
                let matches: Vec<&DisplayPad> = indexed
                    .iter()
                    .filter(|dp| {
                        // Match against title
                        if dp.pad.metadata.title.to_lowercase().contains(&term_lower) {
                            return true;
                        }
                        // Match against content (excluding first line if it duplicates title)
                        // Simplified content match: just check if full content contains it
                        // This matches get.rs logic roughly but simpler
                        dp.pad.content.to_lowercase().contains(&term_lower)
                    })
                    .collect();

                match matches.len() {
                    0 => Err(PadzError::Api(format!("No pad found matching \"{}\"", term))),
                    1 => Ok((matches[0].index.clone(), matches[0].pad.metadata.id)),
                    n => Err(PadzError::Api(format!(
                        "Term \"{}\" matches multiple paths, add more to make it unique(matched {} pads). Please be more specific.",
                        term, n
                    ))),
                }
            }
        })
        .collect()
}

pub fn pads_by_selectors<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<Vec<DisplayPad>> {
    let resolved = resolve_selectors(store, scope, selectors)?;
    let mut pads = Vec::with_capacity(resolved.len());
    for (index, id) in resolved {
        let pad = store.get_pad(&id, scope)?;
        pads.push(DisplayPad {
            pad,
            index,
            matches: None,
        });
    }
    Ok(pads)
}
