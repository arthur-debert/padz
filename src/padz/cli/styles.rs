//! Styles for the Padz CLI application.
//!
//! Padz uses the `outstanding` crate for theming and styling console output, so
//! we keep every style definition in one place. The CLI needs to work equally
//! well in light and dark terminals, so `PADZ_THEME` exposes an adaptive theme
//! that resolves to the appropriate palette at runtime.
//!
//! The shared style tokens are:
//!
//!     * Regular text (neutral foreground that matches the theme background)
//!     * Muted text (used for metadata such as timestamps)
//!     * Faint text (section separators, subtle hints)
//!     * Highlighted text (black on a yellow background for emphasis)
//!     * Pinned elements (yellow accents for icons and indexes)
//!     * Deleted entries (red foreground for both themes)
//!     * Error and warning styles (red / yellow respectively)
//!     * Title text (regular color with added weight)
//!     * Time text (muted + italic)
//!
//! Deleted pads should always render with the `deleted` style, pinned icons use
//! the `pinned` style (icon only), and any time strings go through the `time`
//! style. All of the styles are registered once through
//! `once_cell::sync::Lazy`.
//!
use console::Style;
use once_cell::sync::Lazy;
use outstanding::{rgb_to_ansi256, AdaptiveTheme, Theme};

/// Style identifiers shared between templates and renderers.
pub mod names {
    pub const REGULAR: &str = "regular";
    pub const MUTED: &str = "muted";
    pub const FAINT: &str = "faint";
    pub const HIGHLIGHT: &str = "highlight";
    pub const PINNED: &str = "pinned";
    pub const DELETED: &str = "deleted";
    pub const ERROR: &str = "error";
    pub const WARNING: &str = "warning";
    pub const SUCCESS: &str = "success";
    pub const INFO: &str = "info";
    pub const TITLE: &str = "title";
    pub const TIME: &str = "time";
    // Semantic list styles
    pub const LIST_INDEX: &str = "list-index";
    pub const LIST_TITLE: &str = "list-title";
    pub const DELETED_INDEX: &str = "deleted-index";
    pub const DELETED_TITLE: &str = "deleted-title";
    // Help styles
    pub const HELP_HEADER: &str = "help-header";
    pub const HELP_SECTION: &str = "help-section";
    pub const HELP_COMMAND: &str = "help-command";
    pub const HELP_DESC: &str = "help-desc";
    pub const HELP_USAGE: &str = "help-usage";
}

pub static PADZ_THEME: Lazy<AdaptiveTheme> =
    Lazy::new(|| AdaptiveTheme::new(build_light_theme(), build_dark_theme()));

fn build_light_theme() -> Theme {
    let regular = Style::new().black();
    let muted = Style::new().color256(rgb_to_ansi256((115, 115, 115)));
    let faint = Style::new().color256(rgb_to_ansi256((173, 173, 173)));
    let pinned = Style::new().color256(rgb_to_ansi256((196, 140, 0))).bold();
    let deleted = Style::new().color256(rgb_to_ansi256((186, 33, 45)));
    let warning = Style::new().yellow().bold();
    let error = Style::new().red().bold();
    let success = Style::new().green();
    let info = muted.clone();
    let highlight = Style::new()
        .black()
        .on_color256(rgb_to_ansi256((255, 235, 59)));
    let title = regular.clone().bold();
    let time = muted.clone().italic();
    // Semantic list styles
    let list_index = Style::new().color256(rgb_to_ansi256((196, 140, 0))); // Yellow/gold for regular indexes
    let list_title = regular.clone(); // Normal text for list titles (not bold)
    let deleted_index = Style::new().color256(rgb_to_ansi256((186, 33, 45))); // Red for deleted indexes
    let deleted_title = muted.clone(); // Muted gray for deleted titles
                                       // Help styles
    let help_header = regular.clone().bold();
    let help_section = Style::new().color256(rgb_to_ansi256((196, 140, 0))).bold();
    let help_command = Style::new().color256(rgb_to_ansi256((0, 128, 0)));
    let help_desc = muted.clone();
    let help_usage = Style::new().cyan();

    Theme::new()
        .add(names::REGULAR, regular)
        .add(names::MUTED, muted.clone())
        .add(names::FAINT, faint)
        .add(names::HIGHLIGHT, highlight)
        .add(names::PINNED, pinned)
        .add(names::DELETED, deleted.clone())
        .add(names::ERROR, error)
        .add(names::WARNING, warning)
        .add(names::SUCCESS, success)
        .add(names::INFO, info)
        .add(names::TITLE, title)
        .add(names::TIME, time)
        .add(names::LIST_INDEX, list_index)
        .add(names::LIST_TITLE, list_title)
        .add(names::DELETED_INDEX, deleted_index)
        .add(names::DELETED_TITLE, deleted_title)
        .add(names::HELP_HEADER, help_header)
        .add(names::HELP_SECTION, help_section)
        .add(names::HELP_COMMAND, help_command)
        .add(names::HELP_DESC, help_desc)
        .add(names::HELP_USAGE, help_usage)
}

fn build_dark_theme() -> Theme {
    let regular = Style::new().white();
    let muted = Style::new().color256(rgb_to_ansi256((180, 180, 180)));
    let faint = Style::new().color256(rgb_to_ansi256((110, 110, 110)));
    let pinned = Style::new().color256(rgb_to_ansi256((255, 214, 10))).bold();
    let deleted = Style::new().color256(rgb_to_ansi256((255, 138, 128)));
    let warning = Style::new().yellow().bold();
    let error = Style::new().red().bold();
    let success = Style::new().green();
    let info = muted.clone();
    let highlight = Style::new()
        .black()
        .on_color256(rgb_to_ansi256((229, 185, 0)));
    let title = regular.clone().bold();
    let time = muted.clone().italic();
    // Semantic list styles
    let list_index = Style::new().color256(rgb_to_ansi256((255, 214, 10))); // Yellow for regular indexes
    let list_title = regular.clone(); // Normal text for list titles (not bold)
    let deleted_index = Style::new().color256(rgb_to_ansi256((255, 138, 128))); // Red for deleted indexes
    let deleted_title = muted.clone(); // Muted gray for deleted titles
                                       // Help styles
    let help_header = regular.clone().bold();
    let help_section = Style::new().color256(rgb_to_ansi256((255, 214, 10))).bold();
    let help_command = Style::new().color256(rgb_to_ansi256((144, 238, 144)));
    let help_desc = muted.clone();
    let help_usage = Style::new().cyan();

    Theme::new()
        .add(names::REGULAR, regular)
        .add(names::MUTED, muted.clone())
        .add(names::FAINT, faint)
        .add(names::HIGHLIGHT, highlight)
        .add(names::PINNED, pinned)
        .add(names::DELETED, deleted.clone())
        .add(names::ERROR, error)
        .add(names::WARNING, warning)
        .add(names::SUCCESS, success)
        .add(names::INFO, info)
        .add(names::TITLE, title)
        .add(names::TIME, time)
        .add(names::LIST_INDEX, list_index)
        .add(names::LIST_TITLE, list_title)
        .add(names::DELETED_INDEX, deleted_index)
        .add(names::DELETED_TITLE, deleted_title)
        .add(names::HELP_HEADER, help_header)
        .add(names::HELP_SECTION, help_section)
        .add(names::HELP_COMMAND, help_command)
        .add(names::HELP_DESC, help_desc)
        .add(names::HELP_USAGE, help_usage)
}
