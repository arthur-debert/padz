use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::PadSelector;
use crate::model::Scope;
use crate::store::DataStore;

use super::helpers::pads_by_selectors;

pub fn run<S: DataStore>(store: &S, scope: Scope, selectors: &[PadSelector]) -> Result<CmdResult> {
    let pads = pads_by_selectors(store, scope, selectors)?;
    Ok(CmdResult::default().with_listed_pads(pads))
}
