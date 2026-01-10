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
//! - **Light/dark support**: Only the visual layer differs between themes
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

use console::Style;
use once_cell::sync::Lazy;
use outstanding::{rgb_to_ansi256, AdaptiveTheme, Theme};

/// Semantic style names for use in templates and renderers.
///
/// These are the ONLY style names that should be used in templates.
/// All names describe WHAT is being displayed, not HOW it looks.
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

/// The adaptive theme for padz, containing both light and dark variants.
/// Note: For rendering, use `get_resolved_theme()` which auto-detects the mode.
#[allow(dead_code)]
pub static PADZ_THEME: Lazy<AdaptiveTheme> =
    Lazy::new(|| AdaptiveTheme::new(build_light_theme(), build_dark_theme()));

/// Returns the resolved theme based on the current terminal color mode.
/// Uses the dark-light crate to detect light/dark mode automatically.
pub fn get_resolved_theme() -> Theme {
    match dark_light::detect() {
        dark_light::Mode::Light => build_light_theme(),
        dark_light::Mode::Dark => build_dark_theme(),
    }
}

fn build_light_theme() -> Theme {
    // =========================================================================
    // VISUAL LAYER - Concrete styles with actual colors
    // These are the raw building blocks, prefixed with _ to indicate internal use
    // =========================================================================
    let primary = Style::new().black();
    let gray = Style::new().color256(rgb_to_ansi256((115, 115, 115)));
    let gray_light = Style::new().color256(rgb_to_ansi256((173, 173, 173)));
    let gold = Style::new().color256(rgb_to_ansi256((196, 140, 0)));
    let red = Style::new().color256(rgb_to_ansi256((186, 33, 45)));
    let green = Style::new().color256(rgb_to_ansi256((0, 128, 0)));
    let yellow_bg = Style::new()
        .black()
        .on_color256(rgb_to_ansi256((255, 235, 59)));

    Theme::new()
        // Visual layer - concrete styles (internal)
        .add("_primary", primary.clone())
        .add("_gray", gray.clone())
        .add("_gray_light", gray_light.clone())
        .add("_gold", gold.clone())
        .add("_red", red.clone())
        .add("_green", green.clone())
        .add("_yellow_bg", yellow_bg)
        // =====================================================================
        // PRESENTATION LAYER - Cross-cutting visual concepts (aliases)
        // These provide consistent appearance for similar elements
        // =====================================================================
        .add("_secondary", "_gray")
        .add("_tertiary", "_gray_light")
        .add("_accent", "_gold")
        .add("_danger", "_red")
        .add("_success", "_green")
        // =====================================================================
        // SEMANTIC LAYER - What templates use
        // Some are aliases, some are concrete (when modifiers like bold/italic needed)
        // =====================================================================
        // Core semantic styles (concrete - need modifiers)
        .add(names::TITLE, primary.clone().bold())
        .add(names::TIME, gray.clone().italic())
        .add(names::HINT, "_tertiary")
        // List styles
        .add(names::LIST_INDEX, "_accent")
        .add(names::LIST_TITLE, "_primary")
        .add(names::PINNED, gold.clone().bold())
        .add(names::DELETED, "_danger")
        .add(names::DELETED_INDEX, "_danger")
        .add(names::DELETED_TITLE, "_secondary")
        .add(names::STATUS_ICON, "_secondary")
        // Search/highlight
        .add(names::HIGHLIGHT, "_yellow_bg")
        .add(names::MATCH, "_yellow_bg")
        // Message styles (concrete - need modifiers for emphasis)
        .add(names::ERROR, red.clone().bold())
        .add(names::WARNING, gold.clone().bold())
        .add(names::SUCCESS, "_success")
        .add(names::INFO, "_secondary")
        // Help styles
        .add(names::HELP_HEADER, primary.clone().bold())
        .add(names::HELP_SECTION, gold.clone().bold())
        .add(names::HELP_COMMAND, "_success")
        .add(names::HELP_DESC, "_secondary")
        .add(names::HELP_USAGE, Style::new().cyan())
        // Template content styles (aliases to presentation layer)
        .add(names::HELP_TEXT, "_tertiary")
        .add(names::SECTION_HEADER, "_secondary")
        .add(names::EMPTY_MESSAGE, "_secondary")
        .add(names::PREVIEW, "_tertiary")
        .add(names::TRUNCATION, "_secondary")
        .add(names::LINE_NUMBER, "_secondary")
        .add(names::SEPARATOR, "_tertiary")
}

fn build_dark_theme() -> Theme {
    // =========================================================================
    // VISUAL LAYER - Concrete styles with actual colors (dark mode values)
    // =========================================================================
    let primary = Style::new().white();
    let gray = Style::new().color256(rgb_to_ansi256((180, 180, 180)));
    let gray_light = Style::new().color256(rgb_to_ansi256((110, 110, 110)));
    let gold = Style::new().color256(rgb_to_ansi256((255, 214, 10)));
    let red = Style::new().color256(rgb_to_ansi256((255, 138, 128)));
    let green = Style::new().color256(rgb_to_ansi256((144, 238, 144)));
    let yellow_bg = Style::new()
        .black()
        .on_color256(rgb_to_ansi256((229, 185, 0)));

    Theme::new()
        // Visual layer - concrete styles (internal)
        .add("_primary", primary.clone())
        .add("_gray", gray.clone())
        .add("_gray_light", gray_light.clone())
        .add("_gold", gold.clone())
        .add("_red", red.clone())
        .add("_green", green.clone())
        .add("_yellow_bg", yellow_bg)
        // =====================================================================
        // PRESENTATION LAYER - Cross-cutting visual concepts (aliases)
        // =====================================================================
        .add("_secondary", "_gray")
        .add("_tertiary", "_gray_light")
        .add("_accent", "_gold")
        .add("_danger", "_red")
        .add("_success", "_green")
        // =====================================================================
        // SEMANTIC LAYER - What templates use
        // =====================================================================
        // Core semantic styles (concrete - need modifiers)
        .add(names::TITLE, primary.clone().bold())
        .add(names::TIME, gray.clone().italic())
        .add(names::HINT, "_tertiary")
        // List styles
        .add(names::LIST_INDEX, "_accent")
        .add(names::LIST_TITLE, "_primary")
        .add(names::PINNED, gold.clone().bold())
        .add(names::DELETED, "_danger")
        .add(names::DELETED_INDEX, "_danger")
        .add(names::DELETED_TITLE, "_secondary")
        .add(names::STATUS_ICON, "_secondary")
        // Search/highlight
        .add(names::HIGHLIGHT, "_yellow_bg")
        .add(names::MATCH, "_yellow_bg")
        // Message styles (concrete - need modifiers for emphasis)
        .add(names::ERROR, red.clone().bold())
        .add(names::WARNING, gold.clone().bold())
        .add(names::SUCCESS, "_success")
        .add(names::INFO, "_secondary")
        // Help styles
        .add(names::HELP_HEADER, primary.clone().bold())
        .add(names::HELP_SECTION, gold.clone().bold())
        .add(names::HELP_COMMAND, "_success")
        .add(names::HELP_DESC, "_secondary")
        .add(names::HELP_USAGE, Style::new().cyan())
        // Template content styles (aliases to presentation layer)
        .add(names::HELP_TEXT, "_tertiary")
        .add(names::SECTION_HEADER, "_secondary")
        .add(names::EMPTY_MESSAGE, "_secondary")
        .add(names::PREVIEW, "_tertiary")
        .add(names::TRUNCATION, "_secondary")
        .add(names::LINE_NUMBER, "_secondary")
        .add(names::SEPARATOR, "_tertiary")
}
