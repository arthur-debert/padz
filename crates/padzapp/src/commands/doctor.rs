use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::model::Scope;
use crate::store::DataStore;

pub fn run<S: DataStore>(store: &mut S, scope: Scope) -> Result<CmdResult> {
    let report = store.doctor(scope)?;
    let mut result = CmdResult::default();

    if report.fixed_missing_files == 0 && report.recovered_files == 0 {
        result.add_message(CmdMessage::success("No inconsistencies found."));
    } else {
        result.add_message(CmdMessage::warning("Inconsistencies found and fixed:"));
        if report.fixed_missing_files > 0 {
            result.add_message(CmdMessage::info(format!(
                "  - Removed {} pad(s) listed in DB but missing from disk.",
                report.fixed_missing_files
            )));
        }
        if report.recovered_files > 0 {
            result.add_message(CmdMessage::success(format!(
                "  - Recovered {} pad(s) found on disk but missing from DB.",
                report.recovered_files
            )));
        }
    }

    Ok(result)
}
