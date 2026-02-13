//! # Configuration
//!
//! Padz configuration is managed by [`clapfig`], which handles layered loading
//! from TOML files, environment variables, and programmatic overrides.
//!
//! ## Storage Hierarchy
//!
//! Configuration is resolved in priority order:
//! 1. **Environment variables**: `PADZ__FILE_EXT`, `PADZ__IMPORT_EXTENSIONS`, etc.
//! 2. **Project Config**: `.padz/padz.toml` — Overrides everything for this repo.
//! 3. **Global Config**: OS-appropriate config directory (via `directories` crate).
//! 4. **Compiled Defaults**: Built-in fallbacks via `#[config(default = ...)]`.
//!
//! ## Available Settings
//!
//! | Key | Default | Description |
//! |-----|---------|-------------|
//! | `file_ext` | `.txt` | Extension for new pad files (e.g., `.md`, `.txt`) |
//! | `import_extensions` | `[".md", ".txt", ".text", ".lex"]` | Extensions for `padz import` |
//!
//! ## CLI Usage
//!
//! - `padz config` — Show all configuration values.
//! - `padz config get <key>` — Get a specific value.
//! - `padz config set <key> <value>` — Set a value.
//! - `padz config unset <key>` — Remove a persisted override.
//! - `padz config gen` — Generate a sample `padz.toml`.

use confique::Config;
use serde::{Deserialize, Serialize};

fn default_import_ext() -> Vec<String> {
    vec![
        ".md".to_string(),
        ".txt".to_string(),
        ".text".to_string(),
        ".lex".to_string(),
    ]
}

/// Configuration for padz, stored in `padz.toml`.
#[derive(Config, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PadzConfig {
    /// Extension for new pad files (e.g., ".txt", ".md", ".rs")
    #[config(default = ".txt")]
    pub file_ext: String,

    /// Extensions to look for when importing directories (e.g. ".md", ".txt").
    /// When absent, defaults to [".md", ".txt", ".text", ".lex"].
    pub import_extensions: Option<Vec<String>>,
}

impl Default for PadzConfig {
    fn default() -> Self {
        Self {
            file_ext: ".txt".to_string(),
            import_extensions: None,
        }
    }
}

impl PadzConfig {
    /// Get the file extension, normalized to start with a dot.
    pub fn file_ext(&self) -> String {
        if self.file_ext.starts_with('.') {
            self.file_ext.clone()
        } else {
            format!(".{}", self.file_ext)
        }
    }

    /// Get import extensions, using defaults if not configured.
    pub fn import_extensions(&self) -> Vec<String> {
        self.import_extensions
            .clone()
            .unwrap_or_else(default_import_ext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PadzConfig::default();
        assert_eq!(config.file_ext, ".txt");
        assert_eq!(
            config.import_extensions(),
            vec![".md", ".txt", ".text", ".lex"]
        );
    }

    #[test]
    fn test_file_ext_normalization_with_dot() {
        let config = PadzConfig {
            file_ext: ".md".to_string(),
            ..Default::default()
        };
        assert_eq!(config.file_ext(), ".md");
    }

    #[test]
    fn test_file_ext_normalization_without_dot() {
        let config = PadzConfig {
            file_ext: "rs".to_string(),
            ..Default::default()
        };
        assert_eq!(config.file_ext(), ".rs");
    }

    #[test]
    fn test_import_extensions_default_when_none() {
        let config = PadzConfig::default();
        assert_eq!(
            config.import_extensions(),
            vec![".md", ".txt", ".text", ".lex"]
        );
    }

    #[test]
    fn test_import_extensions_custom() {
        let config = PadzConfig {
            import_extensions: Some(vec![".py".to_string(), ".js".to_string()]),
            ..Default::default()
        };
        assert_eq!(config.import_extensions(), vec![".py", ".js"]);
    }
}
