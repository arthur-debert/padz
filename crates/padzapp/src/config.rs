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
//! | `file_ext` | `txt` | Extension for new pad files (e.g., `md`, `txt`) |
//! | `import_extensions` | `["md", "txt", "text", "lex"]` | Extensions for `padz import` |
//!
//! ## Extension Convention
//!
//! Extensions are stored **without** leading dots (`md`, not `.md`).
//! Dots are stripped on intake via deserialization and added back by accessor
//! methods ([`PadzConfig::file_ext()`], [`PadzConfig::import_extensions()`]).
//!
//! ## CLI Usage
//!
//! - `padz config` — Show all configuration values.
//! - `padz config get <key>` — Get a specific value.
//! - `padz config set <key> <value>` — Set a value.
//! - `padz config unset <key>` — Remove a persisted override.
//! - `padz config gen` — Generate a sample `padz.toml`.

use confique::Config;
use serde::{Deserialize, Deserializer, Serialize};

/// Strip a leading dot from a string during deserialization.
/// `".md"` → `"md"`, `"md"` → `"md"`.
fn strip_leading_dot<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s = String::deserialize(d)?;
    Ok(s.strip_prefix('.').unwrap_or(&s).to_string())
}

fn default_import_ext() -> Vec<String> {
    vec![
        "md".to_string(),
        "txt".to_string(),
        "text".to_string(),
        "lex".to_string(),
    ]
}

/// Configuration for padz, stored in `padz.toml`.
#[derive(Config, Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct PadzConfig {
    /// Extension for new pad files (e.g., "txt", "md", "rs").
    /// Stored without leading dot; use file_ext() for the dotted form.
    #[config(deserialize_with = strip_leading_dot, default = "txt")]
    #[serde(deserialize_with = "strip_leading_dot")]
    pub file_ext: String,

    /// Extensions to look for when importing directories (e.g. "md", "txt").
    /// Stored without leading dots; use import_extensions() for the dotted form.
    /// When absent, defaults to ["md", "txt", "text", "lex"].
    pub import_extensions: Option<Vec<String>>,
}

impl Default for PadzConfig {
    fn default() -> Self {
        Self {
            file_ext: "txt".to_string(),
            import_extensions: None,
        }
    }
}

impl PadzConfig {
    /// Get the file extension with a leading dot (e.g., `.txt`, `.md`).
    pub fn file_ext(&self) -> String {
        let ext = self.file_ext.strip_prefix('.').unwrap_or(&self.file_ext);
        format!(".{}", ext)
    }

    /// Get import extensions with leading dots (e.g., `.md`, `.txt`),
    /// using defaults if not configured.
    pub fn import_extensions(&self) -> Vec<String> {
        let exts = self
            .import_extensions
            .clone()
            .unwrap_or_else(default_import_ext);
        exts.into_iter()
            .map(|e| {
                let bare = e.strip_prefix('.').unwrap_or(&e);
                format!(".{}", bare)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = PadzConfig::default();
        assert_eq!(config.file_ext, "txt");
        assert_eq!(config.file_ext(), ".txt");
        assert_eq!(
            config.import_extensions(),
            vec![".md", ".txt", ".text", ".lex"]
        );
    }

    #[test]
    fn test_file_ext_stored_without_dot() {
        let config = PadzConfig {
            file_ext: "md".to_string(),
            ..Default::default()
        };
        assert_eq!(config.file_ext, "md");
        assert_eq!(config.file_ext(), ".md");
    }

    #[test]
    fn test_file_ext_accessor_handles_legacy_dot() {
        // Even if a dot slips in, accessor still works correctly
        let config = PadzConfig {
            file_ext: ".rs".to_string(),
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
    fn test_import_extensions_custom_without_dots() {
        let config = PadzConfig {
            import_extensions: Some(vec!["py".to_string(), "js".to_string()]),
            ..Default::default()
        };
        assert_eq!(config.import_extensions(), vec![".py", ".js"]);
    }

    #[test]
    fn test_import_extensions_handles_legacy_dots() {
        let config = PadzConfig {
            import_extensions: Some(vec![".py".to_string(), "js".to_string()]),
            ..Default::default()
        };
        assert_eq!(config.import_extensions(), vec![".py", ".js"]);
    }

    #[test]
    fn test_strip_leading_dot_deserializer() {
        // Simulate what confique does: deserialize a TOML string through our normalizer
        let toml_str = r#"file_ext = ".md""#;
        let config: PadzConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.file_ext, "md");
    }

    #[test]
    fn test_strip_leading_dot_deserializer_no_dot() {
        let toml_str = r#"file_ext = "md""#;
        let config: PadzConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.file_ext, "md");
    }
}
