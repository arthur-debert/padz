//! Styles for the Padz CLI.
//!
//! Styles are defined in `styles/default.yaml` using a three-layer architecture.
//! See that file for the full style reference.
//!
//! Stylesheets are embedded at compile time using the `embed_styles!` macro.

use once_cell::sync::Lazy;
use outstanding::{embed_styles, Theme};

/// The default theme, embedded at compile time.
pub static DEFAULT_THEME: Lazy<Theme> = Lazy::new(|| {
    let mut registry = embed_styles!("src/styles");
    registry
        .get("default")
        .expect("Failed to load default theme")
});
