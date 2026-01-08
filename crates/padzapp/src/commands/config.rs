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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn make_paths(dir: &std::path::Path) -> PadzPaths {
        PadzPaths {
            project: Some(dir.to_path_buf()),
            global: dir.to_path_buf(),
        }
    }

    #[test]
    fn show_all_returns_default_config_when_no_file() {
        let temp = tempdir().unwrap();
        let paths = make_paths(temp.path());

        let result = run(&paths, Scope::Project, ConfigAction::ShowAll).unwrap();

        assert!(result.config.is_some());
        let config = result.config.unwrap();
        assert_eq!(config.file_ext, ".txt");
    }

    #[test]
    fn show_all_returns_saved_config() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path()).unwrap();

        // Save a custom config
        let mut config = PadzConfig::default();
        config.set_file_ext(".md");
        config.save(temp.path()).unwrap();

        let paths = make_paths(temp.path());
        let result = run(&paths, Scope::Project, ConfigAction::ShowAll).unwrap();

        assert!(result.config.is_some());
        assert_eq!(result.config.unwrap().file_ext, ".md");
    }

    #[test]
    fn show_key_returns_value_for_known_key() {
        let temp = tempdir().unwrap();
        let paths = make_paths(temp.path());

        let result = run(
            &paths,
            Scope::Project,
            ConfigAction::ShowKey("file-ext".to_string()),
        )
        .unwrap();

        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].content.contains(".txt"));
    }

    #[test]
    fn show_key_returns_error_for_unknown_key() {
        let temp = tempdir().unwrap();
        let paths = make_paths(temp.path());

        let result = run(
            &paths,
            Scope::Project,
            ConfigAction::ShowKey("nonexistent-key".to_string()),
        )
        .unwrap();

        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].content.contains("Unknown config key"));
        assert!(result.messages[0].content.contains("nonexistent-key"));
    }

    #[test]
    fn set_valid_key_saves_and_returns_success() {
        let temp = tempdir().unwrap();
        let paths = make_paths(temp.path());

        let result = run(
            &paths,
            Scope::Project,
            ConfigAction::Set("file-ext".to_string(), ".rs".to_string()),
        )
        .unwrap();

        // Check success message
        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].content.contains("file-ext set to"));
        assert!(result.messages[0].content.contains(".rs"));

        // Check config is updated
        assert!(result.config.is_some());
        assert_eq!(result.config.unwrap().file_ext, ".rs");

        // Verify persisted
        let loaded = PadzConfig::load(temp.path()).unwrap();
        assert_eq!(loaded.file_ext, ".rs");
    }

    #[test]
    fn set_import_extensions_parses_csv() {
        let temp = tempdir().unwrap();
        let paths = make_paths(temp.path());

        let result = run(
            &paths,
            Scope::Project,
            ConfigAction::Set("import-extensions".to_string(), ".py, .js, .ts".to_string()),
        )
        .unwrap();

        assert!(result.config.is_some());
        let config = result.config.unwrap();
        assert_eq!(config.import_extensions, vec![".py", ".js", ".ts"]);

        // Verify persisted
        let loaded = PadzConfig::load(temp.path()).unwrap();
        assert_eq!(loaded.import_extensions, vec![".py", ".js", ".ts"]);
    }

    #[test]
    fn set_invalid_key_returns_error_message() {
        let temp = tempdir().unwrap();
        let paths = make_paths(temp.path());

        let result = run(
            &paths,
            Scope::Project,
            ConfigAction::Set("invalid-key".to_string(), "value".to_string()),
        )
        .unwrap();

        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].content.contains("Unknown config key"));
        assert!(result.messages[0].content.contains("invalid-key"));

        // Config should not be attached on error
        assert!(result.config.is_none());
    }

    #[test]
    fn uses_global_scope_when_specified() {
        let temp = tempdir().unwrap();
        let global_dir = temp.path().join("global");
        fs::create_dir_all(&global_dir).unwrap();

        let paths = PadzPaths {
            project: None,
            global: global_dir.clone(),
        };

        // Set in global scope
        let result = run(
            &paths,
            Scope::Global,
            ConfigAction::Set("file-ext".to_string(), ".global".to_string()),
        )
        .unwrap();

        assert!(result.config.is_some());
        assert_eq!(result.config.unwrap().file_ext, ".global");

        // Verify persisted in global dir
        let loaded = PadzConfig::load(&global_dir).unwrap();
        assert_eq!(loaded.file_ext, ".global");
    }

    #[test]
    fn project_scope_fails_when_unavailable() {
        let temp = tempdir().unwrap();

        let paths = PadzPaths {
            project: None,
            global: temp.path().to_path_buf(),
        };

        let result = run(&paths, Scope::Project, ConfigAction::ShowAll);

        assert!(result.is_err());
    }
}
