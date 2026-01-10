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

    // Add shell completion hint
    result.add_message(CmdMessage::info(String::new())); // blank line
    result.add_message(CmdMessage::info(
        "Tip: Enable shell completions for padz:".to_string(),
    ));
    result.add_message(CmdMessage::info(
        "  eval \"$(padz completions bash)\"  # add to ~/.bashrc".to_string(),
    ));
    result.add_message(CmdMessage::info(
        "  eval \"$(padz completions zsh)\"   # add to ~/.zshrc".to_string(),
    ));

    Ok(result)
}
