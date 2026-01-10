use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::DataStore;

use super::helpers::pads_by_selectors;

pub fn run<S: DataStore>(store: &S, scope: Scope, selectors: &[PadSelector]) -> Result<CmdResult> {
    let pads = pads_by_selectors(store, scope, selectors, false)?;

    // Collect paths for each pad (for editor integration)
    let paths: Vec<_> = pads
        .iter()
        .filter_map(|dp| store.get_pad_path(&dp.pad.metadata.id, scope).ok())
        .collect();

    let mut result = CmdResult::default().with_listed_pads(pads);
    result.pad_paths = paths;
    Ok(result)
}
