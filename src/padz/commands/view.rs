use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::DisplayIndex;
use crate::model::Scope;
use crate::store::DataStore;

use super::helpers::pads_by_indexes;

pub fn run<S: DataStore>(store: &S, scope: Scope, indexes: &[DisplayIndex]) -> Result<CmdResult> {
    let pads = pads_by_indexes(store, scope, indexes)?;
    Ok(CmdResult::default().with_listed_pads(pads))
}
