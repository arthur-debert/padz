//! Styles for the Padz CLI.
//!
//! Styles are defined in `styles/default.yaml` using a three-layer architecture.
//! See that file for the full style reference.

use outstanding::Theme;

/// The default stylesheet, embedded at compile time.
const DEFAULT_STYLESHEET: &str = include_str!("../styles/default.yaml");

/// Returns the resolved theme based on the current terminal color mode.
pub fn get_resolved_theme() -> Theme {
    Theme::from_yaml(DEFAULT_STYLESHEET).expect("Failed to parse embedded stylesheet")
}
