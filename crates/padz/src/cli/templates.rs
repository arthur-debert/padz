//! CLI Templates Module
//!
//! Templates are located in `src/cli/templates/` and use the `.jinja` extension.
//! They are embedded at compile time using the `embed_templates!` macro.
//! In debug builds, files are read from disk for hot-reload; in release builds,
//! embedded content is used.
//!
//! ## Template Naming
//!
//! - Main templates: `list`, `full_pad`, `text_list`, `messages`
//! - Partials (included via `{% include %}`): prefixed with `_` like `_pad_line`, `_deleted_help`

use once_cell::sync::Lazy;
use outstanding::{embed_templates, TemplateRegistry};

/// Embedded templates, compiled into the binary.
pub static TEMPLATES: Lazy<TemplateRegistry> =
    Lazy::new(|| embed_templates!("src/cli/templates").into());
