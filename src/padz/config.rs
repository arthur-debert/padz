use crate::error::{PadzError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

const CONFIG_FILENAME: &str = "config.json";
const DEFAULT_FILE_EXT: &str = ".txt";

/// Configuration for padz, stored in .padz/config.json
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PadzConfig {
    /// File extension for new pads (e.g., ".txt", ".md", ".rs")
    #[serde(default = "default_file_ext")]
    pub file_ext: String,

    /// Extensions to look for when importing directories (e.g. ".md", ".txt")
    #[serde(default = "default_import_ext")]
    pub import_extensions: Vec<String>,
}

fn default_file_ext() -> String {
    DEFAULT_FILE_EXT.to_string()
}

fn default_import_ext() -> Vec<String> {
    vec![
        ".md".to_string(),
        ".txt".to_string(),
        ".text".to_string(),
        ".lex".to_string(),
    ]
}

impl Default for PadzConfig {
    fn default() -> Self {
        Self {
            file_ext: DEFAULT_FILE_EXT.to_string(),
            import_extensions: default_import_ext(),
        }
    }
}

impl PadzConfig {
    /// Load config from the given directory, or return defaults if not found
    pub fn load<P: AsRef<Path>>(config_dir: P) -> Result<Self> {
        let config_path = config_dir.as_ref().join(CONFIG_FILENAME);

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&config_path).map_err(PadzError::Io)?;
        let config: PadzConfig =
            serde_json::from_str(&content).map_err(PadzError::Serialization)?;
        Ok(config)
    }

    /// Save config to the given directory
    pub fn save<P: AsRef<Path>>(&self, config_dir: P) -> Result<()> {
        let config_dir = config_dir.as_ref();

        // Ensure directory exists
        if !config_dir.exists() {
            fs::create_dir_all(config_dir).map_err(PadzError::Io)?;
        }

        let config_path = config_dir.join(CONFIG_FILENAME);
        let content = serde_json::to_string_pretty(self).map_err(PadzError::Serialization)?;
        fs::write(config_path, content).map_err(PadzError::Io)?;
        Ok(())
    }

    /// Get the file extension (ensures it starts with a dot)
    pub fn get_file_ext(&self) -> &str {
        &self.file_ext
    }

    /// Set the file extension (normalizes to start with a dot)
    pub fn set_file_ext(&mut self, ext: &str) {
        if ext.starts_with('.') {
            self.file_ext = ext.to_string();
        } else {
            self.file_ext = format!(".{}", ext);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_default_config() {
        let config = PadzConfig::default();
        assert_eq!(config.file_ext, ".txt");
    }

    #[test]
    fn test_set_file_ext_with_dot() {
        let mut config = PadzConfig::default();
        config.set_file_ext(".md");
        assert_eq!(config.file_ext, ".md");
    }

    #[test]
    fn test_set_file_ext_without_dot() {
        let mut config = PadzConfig::default();
        config.set_file_ext("rs");
        assert_eq!(config.file_ext, ".rs");
    }

    #[test]
    fn test_load_missing_config() {
        let temp_dir = env::temp_dir().join("padz_test_config_missing");
        let _ = fs::remove_dir_all(&temp_dir);

        let config = PadzConfig::load(&temp_dir).unwrap();
        assert_eq!(config, PadzConfig::default());
    }

    #[test]
    fn test_save_and_load() {
        let temp_dir = env::temp_dir().join("padz_test_config_save");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        let mut config = PadzConfig::default();
        config.set_file_ext(".md");
        config.save(&temp_dir).unwrap();

        let loaded = PadzConfig::load(&temp_dir).unwrap();
        assert_eq!(loaded.file_ext, ".md");

        // Cleanup
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let config = PadzConfig {
            file_ext: ".py".to_string(),
            import_extensions: vec![".md".to_string()],
        };

        let json = serde_json::to_string(&config).unwrap();
        let parsed: PadzConfig = serde_json::from_str(&json).unwrap();

        assert_eq!(config, parsed);
    }
}
