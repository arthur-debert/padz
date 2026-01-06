use crate::commands::{CmdMessage, CmdResult, PadzPaths};
use crate::config::PadzConfig;
use crate::error::Result;
use crate::model::Scope;

#[derive(Debug, Clone)]
pub enum ConfigAction {
    ShowAll,
    ShowKey(String),
    Set(String, String),
}

pub fn run(paths: &PadzPaths, scope: Scope, action: ConfigAction) -> Result<CmdResult> {
    let dir = paths.scope_dir(scope)?;
    match action {
        ConfigAction::ShowAll => {
            let config = PadzConfig::load(&dir)?;
            Ok(CmdResult::default().with_config(config))
        }
        ConfigAction::ShowKey(key) => {
            let config = PadzConfig::load(&dir)?;
            let mut result = CmdResult::default();
            match config.get(&key) {
                Some(val) => {
                    result.add_message(CmdMessage::info(val));
                    Ok(result)
                }
                None => {
                    result.add_message(CmdMessage::error(format!("Unknown config key: {}", key)));
                    Ok(result)
                }
            }
        }
        ConfigAction::Set(key, value) => {
            let mut config = PadzConfig::load(&dir)?;
            if let Err(e) = config.set(&key, &value) {
                let mut res = CmdResult::default();
                res.add_message(CmdMessage::error(e));
                return Ok(res);
            }
            config.save(&dir)?;
            let mut result = CmdResult::default().with_config(config.clone());
            // Fetch formatted value back
            let display_val = config.get(&key).unwrap_or_else(|| value.clone());
            result.add_message(CmdMessage::success(format!(
                "{} set to {}",
                key, display_val
            )));
            Ok(result)
        }
    }
}
