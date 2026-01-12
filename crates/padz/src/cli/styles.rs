//! Styles for the Padz CLI.
//!
//! Styles are defined in `styles/default.yaml` using a three-layer architecture.
//! See that file for the full style reference.
//!
//! Stylesheets are embedded at compile time using the `embed_styles!` macro.
//! In debug builds, files are read from disk for hot-reload; in release builds,
//! embedded content is used.

use once_cell::sync::Lazy;
use outstanding::{embed_styles, stylesheet::StylesheetRegistry, Theme};

/// The default theme, embedded at compile time.
pub static DEFAULT_THEME: Lazy<Theme> = Lazy::new(|| {
    let mut registry: StylesheetRegistry = embed_styles!("src/styles").into();
    registry.get("default").expect("default theme")
});
