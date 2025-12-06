use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::index::DisplayIndex;
use crate::model::Scope;
use crate::store::DataStore;
use std::io::{self, Write};

use super::helpers::{indexed_pads, pads_by_indexes};

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    indexes: &[DisplayIndex],
    skip_confirm: bool,
) -> Result<CmdResult> {
    // 1. Resolve targets
    let pads_to_purge = if indexes.is_empty() {
        // If no indexes, target ALL currently deleted pads
        let all_pads = indexed_pads(store, scope)?;
        all_pads
            .into_iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Deleted(_)))
            .collect()
    } else {
        // Specific pads
        pads_by_indexes(store, scope, indexes)?
    };

    if pads_to_purge.is_empty() {
        let mut res = CmdResult::default();
        res.add_message(CmdMessage::info("No pads to purge."));
        return Ok(res);
    }

    // 2. Confirm
    if !skip_confirm {
        println!("This will permanently remove the following pads:");
        for dp in &pads_to_purge {
            println!("  {} {}", dp.index, dp.pad.metadata.title);
        }
        print!("[Y] To delete: ");
        io::stdout().flush().map_err(PadzError::Io)?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).map_err(PadzError::Io)?;

        if input.trim() != "Y" {
            let mut res = CmdResult::default();
            res.add_message(CmdMessage::info("Operation cancelled."));
            return Ok(res);
        }
    }

    // 3. Delete
    let mut result = CmdResult::default();
    for dp in pads_to_purge {
        store.delete_pad(&dp.pad.metadata.id, scope)?;
        result.add_message(CmdMessage::success(format!(
            "Purged: {} {}",
            dp.index, dp.pad.metadata.title
        )));
    }

    Ok(result)
}
