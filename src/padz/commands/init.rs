use crate::commands::{CmdMessage, CmdResult, PadzPaths};
use crate::error::Result;
use crate::model::Scope;
use std::fs;

pub fn run(paths: &PadzPaths, scope: Scope) -> Result<CmdResult> {
    let dir = paths.scope_dir(scope)?;
    fs::create_dir_all(&dir)?;
    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!(
        "Initialized padz store at {}",
        dir.display()
    )));
    Ok(result)
}
