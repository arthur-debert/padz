//! # CLI Templates Module
//!
//! Out output pipleine, oustanding crate based, relies on templates for rendering term output.
//!
//! We have preference for stand-alone templates files, as seprateing them from code makes is easier
//! and safer to edit, diff and so on.
//!
//! Templates are located in `src/cli/templates/` and use the `.tmpl` extension.
//!
//! ## Loading Strategy
//!
//! - **Debug builds**: Templates are loaded from the filesystem on each render, enabling
//!   hot-reload during development. Edit templates without recompiling!
//!
//! - **Release builds**: Templates are embedded at compile time via `include_str!` for
//!   zero filesystem overhead at runtime.
//!
//! ## Template Conventions
//!
//! Templates are minijinja based. A few important best practices:
//!
//!     1. Blank Lines / Whitespace:
//!
//!     While natural to keep templates organized as the output they produce, there are often times
//!     where that forces the template to become unreadble (i.e. many nested conditionals, very long
//!     lines). It can become quite tricky to iterate on blank lines and whitespaces, specilly when
//!     dealing with loops and conditionals.
//!     For this reason, we have templates requiring explicit line breaks, which make it clear where
//!     they are coming from.
//!     2. Reusability and Composition:
//!     Templates can and should be nested when appropriate. This allows for reuse (i.e. a pad
//!     listing title) that can be used in multiple places, and keeps templates smaller and more
//!     readable. Else we descend into a "god output" where everything is defined.
//!
//!     3. Judicial Conditionals:
//!     While conditionals are necessary, they can quickly make templates unreadable.
//!     They are best used when branching what gets output, but not when they contronling styles.
//!
//!     For example, {% if pad.is_pinned %} <pinned-style> {% else %} <regular-style> {% endif %}
//!     throughout various parts in the template.  In this case its best to set the style variable
//!     that does the logic, and then use the style variable directly.
//!
//!     4. Harder Logic
//!     While best avoided, for when more complex logic is needed, it is best to move that logic
//!     into the rust code, and pass the results as functions for the template to use.
//!
//! ## Template Naming
//!
//! - Main templates: `list`, `full_pad`, `text_list`, `messages`
//! - Partials (included via `{% include %}`): prefixed with `_` like `_pad_line`, `_deleted_help`
//!
//! ## Debugging Template Output
//!
//! When developing or testing templates, use the `--output=term-debug` flag to see
//! style names as markup tags instead of ANSI escape codes:
//!
//! ```bash
//! padz list --output=term-debug
//! ```
//!
//! This renders output like:
//! ```text
//! [pinned]⚲[/pinned] [time]⚪︎[/time] [list-index]p1.[/list-index][list-title]My Pad[/list-title]
//! ```
//!
//! This is invaluable for:
//! - Verifying that the correct style is applied to each template element
//! - Debugging layout issues by seeing exactly what styles are where
//! - Writing test assertions that check for specific style applications
//! - Comparing output between template changes without ANSI code noise

use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Embedded templates for release builds.
///
/// In release mode, templates are compiled into the binary to avoid filesystem
/// access at runtime. The HashMap maps template names to their content.
pub static EMBEDDED_TEMPLATES: Lazy<HashMap<String, String>> = Lazy::new(|| {
    let mut map = HashMap::new();

    // Main templates
    map.insert(
        "list".to_string(),
        include_str!("templates/list.tmpl").to_string(),
    );
    map.insert(
        "full_pad".to_string(),
        include_str!("templates/full_pad.tmpl").to_string(),
    );
    map.insert(
        "text_list".to_string(),
        include_str!("templates/text_list.tmpl").to_string(),
    );
    map.insert(
        "messages".to_string(),
        include_str!("templates/messages.tmpl").to_string(),
    );

    // Partial templates (for {% include %} support)
    map.insert(
        "_deleted_help".to_string(),
        include_str!("templates/_deleted_help.tmpl").to_string(),
    );
    map.insert(
        "_peek_content".to_string(),
        include_str!("templates/_peek_content.tmpl").to_string(),
    );
    map.insert(
        "_match_lines".to_string(),
        include_str!("templates/_match_lines.tmpl").to_string(),
    );
    map.insert(
        "_pad_line".to_string(),
        include_str!("templates/_pad_line.tmpl").to_string(),
    );

    map
});
