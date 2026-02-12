//! # Configuration
//!
//! Padz exposes configuration as a first-class command, backed by layered TOML files.
//!
//! ## Storage Hierarchy
//!
//! Configuration is resolved via clapfig with layer precedence (lowest to highest):
//! 1. **Compiled Defaults**: Built-in fallbacks via `#[config(default = ...)]`.
//! 2. **Global Config**: OS-appropriate config directory (via `directories` crate).
//! 3. **Project Config**: `.padz/padz.toml` — Overrides global for this repo.
//!
//! ## Available Settings
//!
//! | Key | Default | Description |
//! |-----|---------|-------------|
//! | `file_ext` | `.txt` | Extension for new pad files (e.g., `.md`, `.txt`) |
//! | `import_extensions` | `[".md", ".txt", ".text", ".lex"]` | Extensions for `padz import` |
//!
//! ## Extension Behavior
//!
//! **`file_ext`**:
//! - Controls the extension for *newly created* files only.
//! - Changing this does **not** rename existing files.
//! - When reading, the store tries the configured extension first, then falls back to `.txt`.
//!
//! **`import_extensions`**:
//! - TOML array of extensions.
//! - Used by `padz import <directory>` to filter which files to import.
//!
//! ## CLI Usage
//!
//! - `padz config` — Show all configuration values.
//! - `padz config get <key>` — Get a specific value.
//! - `padz config set <key> <value>` — Set a value.
//! - `padz config gen` — Generate a commented template.

use confique::Config;
use serde::{Deserialize, Serialize};

/// Configuration for padz, stored in padz.toml
#[derive(Config, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PadzConfig {
    /// File extension for new pads (e.g., ".txt", ".md", ".rs")
    #[config(default = ".txt")]
    pub file_ext: String,

    /// Extensions to look for when importing directories (e.g. ".md", ".txt")
    #[config(default = [".md", ".txt", ".text", ".lex"])]
    pub import_extensions: Vec<String>,
}

impl Default for PadzConfig {
    fn default() -> Self {
        Self {
            file_ext: ".txt".to_string(),
            import_extensions: vec![
                ".md".to_string(),
                ".txt".to_string(),
                ".text".to_string(),
                ".lex".to_string(),
            ],
        }
    }
}

impl PadzConfig {
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

    #[test]
    fn test_default_config() {
        let config = PadzConfig::default();
        assert_eq!(config.file_ext, ".txt");
        assert_eq!(
            config.import_extensions,
            vec![".md", ".txt", ".text", ".lex"]
        );
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
}
