use crate::commands::{CmdMessage, CmdResult, PadzPaths};
use crate::config::PadzConfig;
use crate::error::Result;
use crate::model::Scope;

#[derive(Debug, Clone)]
pub enum ConfigAction {
    ShowAll,
    ShowKey(String),
    SetFileExt(String),
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
            match key.as_str() {
                "file-ext" => {
                    result.add_message(CmdMessage::info(config.get_file_ext().to_string()));
                    Ok(result)
                }
                _ => {
                    result.add_message(CmdMessage::error(format!("Unknown config key: {}", key)));
                    Ok(result)
                }
            }
        }
        ConfigAction::SetFileExt(ext) => {
            let mut config = PadzConfig::load(&dir)?;
            config.set_file_ext(&ext);
            config.save(&dir)?;
            let mut result = CmdResult::default().with_config(config.clone());
            result.add_message(CmdMessage::success(format!(
                "file-ext set to {}",
                config.get_file_ext()
            )));
            Ok(result)
        }
    }
}
