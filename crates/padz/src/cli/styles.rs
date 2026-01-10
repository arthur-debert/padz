//! Styles for the Padz CLI application.
//!
//! Padz uses the `outstanding` crate for theming and styling console output, to separate the data
//! and it's presentaiion layers.
//!
//! We use an adaptattive theme, that is, one that had presentition styles for both light and dark
//! modes, and which the outstanding crate manages automatically.
//!
//! A theme is a collections of named styles, which is a set for formatting optins as in colors, font
//! decoration, weight and so on.
//!
//! Styles : A Three Layer Approach.
//!
//! Keeping in mind that our goal is to keep presentation easy to change, iterate and consistent,
//! styling gets done in three layers.
//!
//! 1. The Application: Semantics (i.e. CSS Classes)
//!
//! On the template, the application is to use semantic style names, that is, names that describe
//! the  data / information being presented. For example a a timestamp with the 'time' style.
//!
//! The semantic style does not define the actual presentation values it self, but rather it refers to
//! the presentation layer.
//!
//! 2. The Presentation Layer: Consistence (i.e. enabled, , focused).
//!
//! The presentation layer defines presentation styles that are consistent accross the application.  
//! These often relates to the data semantics, but often are a cross with state. For example, say that
//! certain elements are disabled. You don't want every disabled element througout the app to look different.
//! Or that an element is highlighted, or focused, etc.
//!
//! And the presentation layer ins't the raw values just yet, but rather another set of named styles,
//! the visual layer.
//!
//! 3. The Visual Layer: Actual Colors and Decorations
//!
//! Now we reach the final point, in which we define the actual colors and decoration values are for
//! presentation styles. Say we define the highlighted has a background color of yellow and black text.
//!
//! This layer gives us flexibility to iterate the visual while keeping the semantics and the presentation
//! consistent. For example light and dark modes only need to define the visual layers differntly,
//! while the  application's code, templates and presentation styles remain the same.
//!
//! The combined result of the three layers is that:
//!    * Templates/ Code work on the semantic level, freening the code from presentation details.
//!    * The presentation layer keeps the application consistent.
//!    * The visual layer allows us to iterate the look and feel easily.
//!
//! The CLI needs to work equally
//! well in light and dark terminals, so `PADZ_THEME` exposes an adaptive theme
//! that resolves to the appropriate palette at runtime.
//!
//! ## Style Reference
//!
//! The following table shows all styles with their three-layer mapping:
//!
//! | Semantic Name     | Presentation | Light Visual                | Dark Visual                 |
//! |-------------------|--------------|-----------------------------|-----------------------------|
//! | `status-icon`     | time         | gray #737373 italic         | gray #B4B4B4 italic         |
//! | `time`            | time         | gray #737373 italic         | gray #B4B4B4 italic         |
//! | `section-header`  | muted        | gray #737373                | gray #B4B4B4                |
//! | `help-text`       | faint        | light gray #ADADAD          | dark gray #6E6E6E           |
//! | `empty-message`   | muted        | gray #737373                | gray #B4B4B4                |
//! | `preview`         | faint        | light gray #ADADAD          | dark gray #6E6E6E           |
//! | `truncation`      | muted        | gray #737373                | gray #B4B4B4                |
//! | `line-number`     | muted        | gray #737373                | gray #B4B4B4                |
//! | `separator`       | faint        | light gray #ADADAD          | dark gray #6E6E6E           |
//! | `list-index`      | —            | gold #C48C00                | yellow #FFD60A              |
//! | `list-title`      | regular      | black                       | white                       |
//! | `deleted-index`   | deleted      | red #BA212D                 | salmon #FF8A80              |
//! | `deleted-title`   | muted        | gray #737373                | gray #B4B4B4                |
//! | `pinned`          | —            | gold #C48C00 bold           | yellow #FFD60A bold         |
//! | `title`           | regular+bold | black bold                  | white bold                  |
//! | `regular`         | —            | black                       | white                       |
//! | `muted`           | —            | gray #737373                | gray #B4B4B4                |
//! | `faint`           | —            | light gray #ADADAD          | dark gray #6E6E6E           |
//! | `highlight`       | —            | black on yellow #FFEB3B     | black on gold #E5B900       |
//! | `deleted`         | —            | red #BA212D                 | salmon #FF8A80              |
//! | `error`           | —            | red bold                    | red bold                    |
//! | `warning`         | —            | yellow bold                 | yellow bold                 |
//! | `success`         | —            | green                       | green                       |
//! | `info`            | muted        | gray #737373                | gray #B4B4B4                |
//!
//! ### Help Command Styles
//!
//! | Semantic Name  | Presentation | Light Visual           | Dark Visual            |
//! |----------------|--------------|------------------------|------------------------|
//! | `help-header`  | regular+bold | black bold             | white bold             |
//! | `help-section` | —            | gold #C48C00 bold      | yellow #FFD60A bold    |
//! | `help-command` | —            | green #008000          | light green #90EE90    |
//! | `help-desc`    | muted        | gray #737373           | gray #B4B4B4           |
//! | `help-usage`   | —            | cyan                   | cyan                   |
//!
//! ## Debugging Styled Output
//!
//! When developing or testing templates and styles, use the `--output=term-debug` flag
//! to see style names as markup tags instead of ANSI escape codes:
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
//! This makes it easy to verify which styles are applied to each element, debug
//! template issues, and write assertions in tests without dealing with ANSI codes.
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
    // Semantic styles for template content
    pub const STATUS_ICON: &str = "status-icon";
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
    // Semantic styles - map to presentation styles
    let status_icon = time.clone(); // Same as time (muted+italic)
    let help_text = faint.clone(); // Same as faint
    let section_header = muted.clone(); // Same as muted
    let empty_message = muted.clone(); // Same as muted
    let preview = faint.clone(); // Same as faint
    let truncation = muted.clone(); // Same as muted
    let line_number = muted.clone(); // Same as muted
    let separator = faint.clone(); // Same as faint

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
        .add(names::STATUS_ICON, status_icon)
        .add(names::HELP_TEXT, help_text)
        .add(names::SECTION_HEADER, section_header)
        .add(names::EMPTY_MESSAGE, empty_message)
        .add(names::PREVIEW, preview)
        .add(names::TRUNCATION, truncation)
        .add(names::LINE_NUMBER, line_number)
        .add(names::SEPARATOR, separator)
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
    // Semantic styles - map to presentation styles
    let status_icon = time.clone(); // Same as time (muted+italic)
    let help_text = faint.clone(); // Same as faint
    let section_header = muted.clone(); // Same as muted
    let empty_message = muted.clone(); // Same as muted
    let preview = faint.clone(); // Same as faint
    let truncation = muted.clone(); // Same as muted
    let line_number = muted.clone(); // Same as muted
    let separator = faint.clone(); // Same as faint

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
        .add(names::STATUS_ICON, status_icon)
        .add(names::HELP_TEXT, help_text)
        .add(names::SECTION_HEADER, section_header)
        .add(names::EMPTY_MESSAGE, empty_message)
        .add(names::PREVIEW, preview)
        .add(names::TRUNCATION, truncation)
        .add(names::LINE_NUMBER, line_number)
        .add(names::SEPARATOR, separator)
}
