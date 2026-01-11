//! # Styles for the Padz CLI
//!
//! This module implements a **three-layer styling architecture** using the `outstanding` crate's
//! style aliasing feature. This design separates concerns and makes it easy to iterate on the
//! visual appearance without touching templates or application code.
//!
//! ## The Three Layers
//!
//! ### 1. Visual Layer (Private)
//!
//! The foundation layer defines concrete `console::Style` values with actual colors and
//! decorations. These are registered with `_` prefix to indicate they're internal:
//!
//! - `_primary` - Main text color (black for light, white for dark)
//! - `_gray` - Secondary text color
//! - `_gray_light` - Tertiary/hint text color
//! - `_gold` - Accent color (yellow/gold)
//! - `_red` - Danger/error color
//! - `_green` - Success color
//! - `_yellow_bg` - Highlight background
//!
//! ### 2. Presentation Layer (Internal)
//!
//! This layer defines cross-cutting visual concepts as aliases to visual styles:
//!
//! - `_secondary` -> `_gray` (less prominent text)
//! - `_tertiary` -> `_gray_light` (subtle hints)
//! - `_accent` -> `_gold` (attention/emphasis)
//! - `_danger` -> `_red` (errors/warnings)
//! - `_success` -> `_green` (positive feedback)
//!
//! ### 3. Semantic Layer (Public)
//!
//! Templates use these semantic names that describe WHAT is being displayed:
//!
//! - `time` - Timestamps
//! - `title` - Pad titles
//! - `list-index` - List item indexes
//! - `pinned` - Pinned markers
//! - `deleted-index` - Deleted item indexes
//! - `hint` - Help text, subtle hints
//! - `error`, `warning`, `success`, `info` - Messages
//!
//! ## Why Three Layers?
//!
//! - **Templates stay clean**: They use semantic names like `time` instead of color codes
//! - **Consistency**: All "secondary" text looks the same across the app
//! - **Easy iteration**: Change `_gray` color and all secondary text updates
//! - **Light/dark support**: Styles are adaptive with light/dark variants
//!
//! ## Style Reference
//!
//! | Semantic Name     | Presentation | Light Visual                | Dark Visual                 |
//! |-------------------|--------------|-----------------------------|-----------------------------|
//! | `status-icon`     | secondary    | gray #737373                | gray #B4B4B4                |
//! | `time`            | secondary    | gray #737373 italic         | gray #B4B4B4 italic         |
//! | `section-header`  | secondary    | gray #737373                | gray #B4B4B4                |
//! | `hint`            | tertiary     | light gray #ADADAD          | dark gray #6E6E6E           |
//! | `empty-message`   | secondary    | gray #737373                | gray #B4B4B4                |
//! | `preview`         | tertiary     | light gray #ADADAD          | dark gray #6E6E6E           |
//! | `truncation`      | secondary    | gray #737373                | gray #B4B4B4                |
//! | `line-number`     | secondary    | gray #737373                | gray #B4B4B4                |
//! | `separator`       | tertiary     | light gray #ADADAD          | dark gray #6E6E6E           |
//! | `list-index`      | accent       | gold #C48C00                | yellow #FFD60A              |
//! | `list-title`      | primary      | black                       | white                       |
//! | `deleted-index`   | danger       | red #BA212D                 | salmon #FF8A80              |
//! | `deleted-title`   | secondary    | gray #737373                | gray #B4B4B4                |
//! | `pinned`          | accent+bold  | gold #C48C00 bold           | yellow #FFD60A bold         |
//! | `title`           | primary+bold | black bold                  | white bold                  |
//! | `highlight`       | yellow_bg    | black on yellow #FFEB3B     | black on gold #E5B900       |
//! | `match`           | yellow_bg    | black on yellow #FFEB3B     | black on gold #E5B900       |
//! | `error`           | danger+bold  | red bold                    | red bold                    |
//! | `warning`         | accent+bold  | yellow bold                 | yellow bold                 |
//! | `success`         | success      | green                       | green                       |
//! | `info`            | secondary    | gray #737373                | gray #B4B4B4                |
//!
//! ### Help Command Styles
//!
//! | Semantic Name  | Presentation | Light Visual           | Dark Visual            |
//! |----------------|--------------|------------------------|------------------------|
//! | `help-header`  | primary+bold | black bold             | white bold             |
//! | `help-section` | accent+bold  | gold #C48C00 bold      | yellow #FFD60A bold    |
//! | `help-command` | success      | green #008000          | light green #90EE90    |
//! | `help-desc`    | secondary    | gray #737373           | gray #B4B4B4           |
//! | `help-usage`   | —            | cyan                   | cyan                   |
//!
//! ## Debugging Styled Output
//!
//! When developing or testing templates and styles, use the `--output=term-debug` flag
//! to see style names as markup tags instead of ANSI escape codes:
//!
//! ```bash
//! padz list --output=term-debug
//! # Output: [pinned]⚲[/pinned] [time]⚪︎[/time] [list-index]p1.[/list-index]
//! ```

use outstanding::Theme;

/// The default stylesheet, embedded at compile time.
const DEFAULT_STYLESHEET: &str = include_str!("../styles/default.yaml");

/// Semantic style names for use in templates and renderers.
///
/// These are the ONLY style names that should be used in templates.
/// All names describe WHAT is being displayed, not HOW it looks.
#[allow(dead_code)]
pub mod names {
    // Core semantic styles
    pub const TITLE: &str = "title";
    pub const TIME: &str = "time";
    pub const HINT: &str = "hint";

    // List styles
    pub const LIST_INDEX: &str = "list-index";
    pub const LIST_TITLE: &str = "list-title";
    pub const PINNED: &str = "pinned";
    pub const DELETED: &str = "deleted";
    pub const DELETED_INDEX: &str = "deleted-index";
    pub const DELETED_TITLE: &str = "deleted-title";
    pub const STATUS_ICON: &str = "status-icon";

    // Search/highlight
    pub const HIGHLIGHT: &str = "highlight";
    pub const MATCH: &str = "match";

    // Message styles
    pub const ERROR: &str = "error";
    pub const WARNING: &str = "warning";
    pub const SUCCESS: &str = "success";
    pub const INFO: &str = "info";

    // Help styles
    pub const HELP_HEADER: &str = "help-header";
    pub const HELP_SECTION: &str = "help-section";
    pub const HELP_COMMAND: &str = "help-command";
    pub const HELP_DESC: &str = "help-desc";
    pub const HELP_USAGE: &str = "help-usage";

    // Template content styles
    pub const HELP_TEXT: &str = "help-text";
    pub const SECTION_HEADER: &str = "section-header";
    pub const EMPTY_MESSAGE: &str = "empty-message";
    pub const PREVIEW: &str = "preview";
    pub const TRUNCATION: &str = "truncation";
    pub const LINE_NUMBER: &str = "line-number";
    pub const SEPARATOR: &str = "separator";
}

/// Returns the resolved theme based on the current terminal color mode.
/// The theme is loaded from the embedded YAML stylesheet and automatically
/// adapts to light/dark mode based on OS detection.
pub fn get_resolved_theme() -> Theme {
    Theme::from_yaml(DEFAULT_STYLESHEET).expect("Failed to parse embedded stylesheet")
}
