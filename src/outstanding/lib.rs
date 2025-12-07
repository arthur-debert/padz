//! # Outstanding - Styled CLI Template Rendering
//!
//! A lightweight system for rendering styled terminal output using templates with
//! automatic terminal capability detection and graceful degradation.
//!
//! ## The Problem
//!
//! CLI applications need styled output (colors, bold, underline) for good UX, but:
//! - Inline ANSI codes in templates are ugly and hard to maintain
//! - Not all terminals support colors (pipes, CI, `TERM=dumb`)
//! - Mixing presentation logic with template structure creates coupling
//!
//! ## The Solution
//!
//! Outstanding separates concerns:
//! - **Templates** define structure using Jinja2 syntax (via minijinja)
//! - **Styles** are defined separately and applied via template filters
//! - **Terminal detection** happens automatically, with graceful degradation
//!
//! ## Quick Example
//!
//! ```rust
//! use outstanding::{render, Theme, ThemeChoice};
//! use console::Style;
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct Data {
//!     name: String,
//!     count: usize,
//! }
//!
//! let theme = Theme::new()
//!     .add("header", Style::new().bold().cyan())
//!     .add("count", Style::new().green());
//!
//! let template = r#"{{ "Results for" | style("header") }} {{ name }}
//! Found {{ count | style("count") }} items"#;
//!
//! let data = Data { name: "test".into(), count: 42 };
//! let output = render(template, &data, ThemeChoice::from(&theme)).unwrap();
//! println!("{}", output);
//! ```
//!
//! ## How It Works
//!
//! 1. Define styles as named `console::Style` objects inside a [`Theme`]
//! 2. Write templates using `{{ value | style("name") }}` filter syntax
//! 3. Call [`render`] with template, data, and either a [`Theme`] or [`AdaptiveTheme`]
//! 4. Outstanding auto-detects terminal capabilities and applies (or skips) ANSI codes
//!
//! ## Terminal Detection
//!
//! Outstanding uses the `console` crate to detect if stdout supports colors.
//! When colors are unsupported (piped output, `TERM=dumb`, etc.), the `style`
//! filter becomes a no-op, returning plain text.
//!
//! You can override detection:
//! - [`render_with_color`]: Force color on/off regardless of detection
//! - [`Renderer::with_color`]: Create a renderer with explicit color setting
//!
//! ## Template Syntax
//!
//! Templates use [minijinja](https://docs.rs/minijinja) (Jinja2-compatible):
//!
//! ```jinja
//! {{ "Header" | style("header") }}
//!
//! {% for item in items %}
//!   {{ item.name | style("name") }}: {{ item.value }}
//! {% endfor %}
//!
//! {{ "Total:" | style("dim") }} {{ count | style("count") }}
//! ```
//!
//! ## Renderer for Multiple Templates
//!
//! For applications with many templates, use [`Renderer`] to pre-register them:
//!
//! ```rust
//! use outstanding::{Renderer, Theme};
//! use console::Style;
//!
//! let theme = Theme::new()
//!     .add("ok", Style::new().green());
//!
//! let mut renderer = Renderer::new(theme);
//! renderer.add_template("status", "Status: {{ msg | style(\"ok\") }}").unwrap();
//!
//! // Later, render by name
//! # use serde::Serialize;
//! # #[derive(Serialize)]
//! # struct StatusData { msg: String }
//! let output = renderer.render("status", &StatusData { msg: "ready".into() }).unwrap();
//! ```
//!
//! ## Integration with clap
//!
//! Works alongside clap with no conflicts:
//!
//! ```rust,ignore
//! use clap::Parser;
//!
//! #[derive(Parser)]
//! struct Cli {
//!     #[arg(long)]
//!     no_color: bool,
//! }
//!
//! let cli = Cli::parse();
//! let output = render_with_color(template, &data, ThemeChoice::from(&theme), !cli.no_color).unwrap();
//! ```

use console::{Style, Term};
use minijinja::{Environment, Error, Value};
use once_cell::sync::Lazy;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;

/// Default prefix shown when a style name is not found.
pub const DEFAULT_MISSING_STYLE_INDICATOR: &str = "(!?)";

/// A collection of named styles.
///
/// Styles are registered by name and applied via the `style` filter in templates.
/// When a style name is not found, a configurable indicator is prepended to the text
/// to help catch typos in templates (defaults to `(!?)`).
///
/// # Example
///
/// ```rust
/// use outstanding::Styles;
/// use console::Style;
///
/// let styles = Styles::new()
///     .add("error", Style::new().bold().red())
///     .add("warning", Style::new().yellow())
///     .add("dim", Style::new().dim());
///
/// // Apply a style (returns styled string)
/// let styled = styles.apply("error", "Something went wrong");
///
/// // Unknown style shows indicator
/// let unknown = styles.apply("typo", "Hello");
/// assert!(unknown.starts_with("(!?)"));
/// ```
#[derive(Clone)]
pub struct Styles {
    styles: HashMap<String, Style>,
    missing_indicator: String,
}

/// A named collection of styles used when rendering templates.
#[derive(Clone)]
pub struct Theme {
    styles: Styles,
}

impl Theme {
    /// Creates an empty theme.
    pub fn new() -> Self {
        Self {
            styles: Styles::new(),
        }
    }

    /// Creates a theme from an existing [`Styles`] collection.
    pub fn from_styles(styles: Styles) -> Self {
        Self { styles }
    }

    /// Adds a named style, returning an updated theme for chaining.
    pub fn add(mut self, name: &str, style: Style) -> Self {
        self.styles = self.styles.add(name, style);
        self
    }

    /// Returns the underlying styles.
    pub fn styles(&self) -> &Styles {
        &self.styles
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new()
    }
}

/// A theme that adapts based on the user's display mode.
#[derive(Clone)]
pub struct AdaptiveTheme {
    light: Theme,
    dark: Theme,
}

impl AdaptiveTheme {
    pub fn new(light: Theme, dark: Theme) -> Self {
        Self { light, dark }
    }

    fn resolve(&self) -> Theme {
        match detect_color_mode() {
            ColorMode::Light => self.light.clone(),
            ColorMode::Dark => self.dark.clone(),
        }
    }
}

/// Reference to either a static theme or an adaptive theme.
pub enum ThemeChoice<'a> {
    Theme(&'a Theme),
    Adaptive(&'a AdaptiveTheme),
}

impl<'a> ThemeChoice<'a> {
    fn resolve(&self) -> Theme {
        match self {
            ThemeChoice::Theme(theme) => (*theme).clone(),
            ThemeChoice::Adaptive(adaptive) => adaptive.resolve(),
        }
    }
}

impl<'a> From<&'a Theme> for ThemeChoice<'a> {
    fn from(theme: &'a Theme) -> Self {
        ThemeChoice::Theme(theme)
    }
}

impl<'a> From<&'a AdaptiveTheme> for ThemeChoice<'a> {
    fn from(adaptive: &'a AdaptiveTheme) -> Self {
        ThemeChoice::Adaptive(adaptive)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Light,
    Dark,
}

type ThemeDetector = fn() -> ColorMode;

static THEME_DETECTOR: Lazy<Mutex<ThemeDetector>> = Lazy::new(|| Mutex::new(|| ColorMode::Light));

/// Overrides the detector used to determine whether the user prefers a light or dark theme.
/// Useful for testing.
pub fn set_theme_detector(detector: ThemeDetector) {
    let mut guard = THEME_DETECTOR.lock().unwrap();
    *guard = detector;
}

fn detect_color_mode() -> ColorMode {
    let detector = THEME_DETECTOR.lock().unwrap();
    (*detector)()
}

impl Default for Styles {
    fn default() -> Self {
        Self {
            styles: HashMap::new(),
            missing_indicator: DEFAULT_MISSING_STYLE_INDICATOR.to_string(),
        }
    }
}

impl Styles {
    /// Creates an empty style registry with the default missing style indicator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a custom indicator to prepend when a style name is not found.
    ///
    /// This helps catch typos in templates. Set to empty string to disable.
    ///
    /// # Example
    ///
    /// ```rust
    /// use outstanding::Styles;
    ///
    /// let styles = Styles::new()
    ///     .missing_indicator("[MISSING]")
    ///     .add("ok", console::Style::new().green());
    ///
    /// // Typo in style name
    /// let output = styles.apply("typo", "Hello");
    /// assert_eq!(output, "[MISSING] Hello");
    /// ```
    pub fn missing_indicator(mut self, indicator: &str) -> Self {
        self.missing_indicator = indicator.to_string();
        self
    }

    /// Adds a named style. Returns self for chaining.
    ///
    /// If a style with the same name exists, it is replaced.
    pub fn add(mut self, name: &str, style: Style) -> Self {
        self.styles.insert(name.to_string(), style);
        self
    }

    /// Applies a named style to text.
    ///
    /// If the style exists, returns the styled string (with ANSI codes).
    /// If not found, prepends the missing indicator (unless it's empty).
    pub fn apply(&self, name: &str, text: &str) -> String {
        match self.styles.get(name) {
            Some(style) => style.apply_to(text).to_string(),
            None if self.missing_indicator.is_empty() => text.to_string(),
            None => format!("{} {}", self.missing_indicator, text),
        }
    }

    /// Applies style checking without ANSI codes (plain text mode).
    ///
    /// If the style exists, returns the text unchanged.
    /// If not found, prepends the missing indicator (unless it's empty).
    pub fn apply_plain(&self, name: &str, text: &str) -> String {
        if self.styles.contains_key(name) || self.missing_indicator.is_empty() {
            text.to_string()
        } else {
            format!("{} {}", self.missing_indicator, text)
        }
    }

    /// Returns true if a style with the given name exists.
    pub fn has(&self, name: &str) -> bool {
        self.styles.contains_key(name)
    }

    /// Returns the number of registered styles.
    pub fn len(&self) -> usize {
        self.styles.len()
    }

    /// Returns true if no styles are registered.
    pub fn is_empty(&self) -> bool {
        self.styles.is_empty()
    }
}

/// Renders a template with automatic terminal color detection.
///
/// This is the simplest way to render styled output. It automatically detects
/// whether stdout supports colors and applies styles accordingly.
///
/// # Arguments
///
/// * `template` - A minijinja template string
/// * `data` - Any serializable data to pass to the template
/// * `theme` - Theme definitions (or adaptive theme) to use for the `style` filter
///
/// # Example
///
/// ```rust
/// use outstanding::{render, Theme, ThemeChoice};
/// use console::Style;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Data { message: String }
///
/// let theme = Theme::new().add("ok", Style::new().green());
/// let output = render(
///     r#"{{ message | style("ok") }}"#,
///     &Data { message: "Success!".into() },
///     ThemeChoice::from(&theme),
/// ).unwrap();
/// ```
pub fn render<T: Serialize>(
    template: &str,
    data: &T,
    theme: ThemeChoice<'_>,
) -> Result<String, Error> {
    let use_color = Term::stdout().features().colors_supported();
    render_with_color(template, data, theme, use_color)
}

/// Renders a template with explicit color control.
///
/// Use this when you need to override automatic terminal detection,
/// for example when honoring a `--no-color` CLI flag.
///
/// # Arguments
///
/// * `template` - A minijinja template string
/// * `data` - Any serializable data to pass to the template
/// * `styles` - Style definitions to use for the `style` filter
/// * `use_color` - Whether to apply ANSI styling (false = plain text)
///
/// # Example
///
/// ```rust
/// use outstanding::{render_with_color, Theme, ThemeChoice};
/// use console::Style;
/// use serde::Serialize;
///
/// #[derive(Serialize)]
/// struct Data { status: String }
///
/// let theme = Theme::new().add("ok", Style::new().green());
///
/// // Force no color (e.g., --no-color flag)
/// let plain = render_with_color(
///     r#"{{ status | style("ok") }}"#,
///     &Data { status: "done".into() },
///     ThemeChoice::from(&theme),
///     false,
/// ).unwrap();
/// assert_eq!(plain, "done"); // No ANSI codes
/// ```
pub fn render_with_color<T: Serialize>(
    template: &str,
    data: &T,
    theme: ThemeChoice<'_>,
    use_color: bool,
) -> Result<String, Error> {
    let theme = theme.resolve();
    let mut env = Environment::new();
    register_style_filter(&mut env, theme, use_color);

    env.add_template_owned("_inline".to_string(), template.to_string())?;
    let tmpl = env.get_template("_inline")?;
    tmpl.render(data)
}

/// A renderer with pre-registered templates.
///
/// Use this when your application has multiple templates that are rendered
/// repeatedly. Templates are compiled once and reused.
///
/// # Example
///
/// ```rust
/// use outstanding::{Renderer, Theme};
/// use console::Style;
/// use serde::Serialize;
///
/// let theme = Theme::new()
///     .add("title", Style::new().bold())
///     .add("count", Style::new().cyan());
///
/// let mut renderer = Renderer::new(theme);
/// renderer.add_template("header", r#"{{ title | style("title") }}"#).unwrap();
/// renderer.add_template("stats", r#"Count: {{ n | style("count") }}"#).unwrap();
///
/// #[derive(Serialize)]
/// struct Header { title: String }
///
/// #[derive(Serialize)]
/// struct Stats { n: usize }
///
/// let h = renderer.render("header", &Header { title: "Report".into() }).unwrap();
/// let s = renderer.render("stats", &Stats { n: 42 }).unwrap();
/// ```
pub struct Renderer {
    env: Environment<'static>,
}

impl Renderer {
    /// Creates a new renderer with automatic color detection.
    pub fn new(theme: Theme) -> Self {
        let use_color = Term::stdout().features().colors_supported();
        Self::with_color(theme, use_color)
    }

    /// Creates a new renderer with explicit color control.
    pub fn with_color(theme: Theme, use_color: bool) -> Self {
        let mut env = Environment::new();
        register_style_filter(&mut env, theme, use_color);
        Self { env }
    }

    /// Registers a named template.
    ///
    /// The template is compiled immediately; errors are returned if syntax is invalid.
    pub fn add_template(&mut self, name: &str, source: &str) -> Result<(), Error> {
        self.env
            .add_template_owned(name.to_string(), source.to_string())
    }

    /// Renders a registered template with the given data.
    ///
    /// # Errors
    ///
    /// Returns an error if the template name is not found or rendering fails.
    pub fn render<T: Serialize>(&self, name: &str, data: &T) -> Result<String, Error> {
        let tmpl = self.env.get_template(name)?;
        tmpl.render(data)
    }
}

/// Registers the `style` filter on a minijinja environment.
fn register_style_filter(env: &mut Environment<'static>, theme: Theme, use_color: bool) {
    let styles = theme.styles.clone();
    env.add_filter("style", move |value: Value, name: String| -> String {
        let text = value.to_string();
        if use_color {
            styles.apply(&name, &text)
        } else {
            // Still check for missing styles even when colors are disabled
            styles.apply_plain(&name, &text)
        }
    });
}

/// Converts an RGB triplet to the nearest ANSI 256-color palette index.
pub fn rgb_to_ansi256((r, g, b): (u8, u8, u8)) -> u8 {
    if r == g && g == b {
        if r < 8 {
            16
        } else if r > 248 {
            231
        } else {
            232 + ((r as u16 - 8) * 24 / 247) as u8
        }
    } else {
        let red = (r as u16 * 5 / 255) as u8;
        let green = (g as u16 * 5 / 255) as u8;
        let blue = (b as u16 * 5 / 255) as u8;
        16 + 36 * red + 6 * green + blue
    }
}

/// Placeholder helper for true-color output; currently returns the RGB triplet unchanged so it
/// can be handed to future true-color aware APIs.
pub fn rgb_to_truecolor(rgb: (u8, u8, u8)) -> (u8, u8, u8) {
    rgb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct SimpleData {
        message: String,
    }

    #[derive(Serialize)]
    struct ListData {
        items: Vec<String>,
        count: usize,
    }

    #[test]
    fn test_styles_new_is_empty() {
        let styles = Styles::new();
        assert!(styles.is_empty());
        assert_eq!(styles.len(), 0);
    }

    #[test]
    fn test_styles_add_and_has() {
        let styles = Styles::new()
            .add("error", Style::new().red())
            .add("ok", Style::new().green());

        assert!(styles.has("error"));
        assert!(styles.has("ok"));
        assert!(!styles.has("warning"));
        assert_eq!(styles.len(), 2);
    }

    #[test]
    fn test_styles_apply_unknown_shows_indicator() {
        let styles = Styles::new();
        let result = styles.apply("nonexistent", "hello");
        assert_eq!(result, "(!?) hello");
    }

    #[test]
    fn test_styles_apply_unknown_with_empty_indicator() {
        let styles = Styles::new().missing_indicator("");
        let result = styles.apply("nonexistent", "hello");
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_styles_apply_unknown_with_custom_indicator() {
        let styles = Styles::new().missing_indicator("[MISSING]");
        let result = styles.apply("nonexistent", "hello");
        assert_eq!(result, "[MISSING] hello");
    }

    #[test]
    fn test_styles_apply_plain_known_style() {
        let styles = Styles::new().add("bold", Style::new().bold());
        let result = styles.apply_plain("bold", "hello");
        // apply_plain returns text without ANSI codes
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_styles_apply_plain_unknown_shows_indicator() {
        let styles = Styles::new();
        let result = styles.apply_plain("nonexistent", "hello");
        assert_eq!(result, "(!?) hello");
    }

    #[test]
    fn test_styles_apply_known_style() {
        let styles = Styles::new().add("bold", Style::new().bold().force_styling(true));
        let result = styles.apply("bold", "hello");
        // The result should contain ANSI codes for bold
        assert!(result.contains("hello"));
        // Bold ANSI code is \x1b[1m
        assert!(result.contains("\x1b[1m"));
    }

    #[test]
    fn test_render_with_color_false_no_ansi() {
        let styles = Styles::new().add("red", Style::new().red());
        let theme = Theme::from_styles(styles);
        let data = SimpleData {
            message: "test".into(),
        };

        let output = render_with_color(
            r#"{{ message | style("red") }}"#,
            &data,
            ThemeChoice::from(&theme),
            false,
        )
        .unwrap();

        assert_eq!(output, "test");
        assert!(!output.contains("\x1b["));
    }

    #[test]
    fn test_render_with_color_true_has_ansi() {
        // Use force_styling to ensure ANSI codes are emitted even in test environment
        let styles = Styles::new().add("green", Style::new().green().force_styling(true));
        let theme = Theme::from_styles(styles);
        let data = SimpleData {
            message: "success".into(),
        };

        let output = render_with_color(
            r#"{{ message | style("green") }}"#,
            &data,
            ThemeChoice::from(&theme),
            true,
        )
        .unwrap();

        assert!(output.contains("success"));
        assert!(output.contains("\x1b[")); // Contains ANSI escape
    }

    #[test]
    fn test_render_unknown_style_shows_indicator() {
        let styles = Styles::new();
        let theme = Theme::from_styles(styles);
        let data = SimpleData {
            message: "hello".into(),
        };

        let output = render_with_color(
            r#"{{ message | style("unknown") }}"#,
            &data,
            ThemeChoice::from(&theme),
            true,
        )
        .unwrap();

        assert_eq!(output, "(!?) hello");
    }

    #[test]
    fn test_render_unknown_style_shows_indicator_no_color() {
        let styles = Styles::new();
        let theme = Theme::from_styles(styles);
        let data = SimpleData {
            message: "hello".into(),
        };

        // Even with colors disabled, missing indicator should appear
        let output = render_with_color(
            r#"{{ message | style("unknown") }}"#,
            &data,
            ThemeChoice::from(&theme),
            false,
        )
        .unwrap();

        assert_eq!(output, "(!?) hello");
    }

    #[test]
    fn test_render_unknown_style_silent_with_empty_indicator() {
        let styles = Styles::new().missing_indicator("");
        let theme = Theme::from_styles(styles);
        let data = SimpleData {
            message: "hello".into(),
        };

        let output = render_with_color(
            r#"{{ message | style("unknown") }}"#,
            &data,
            ThemeChoice::from(&theme),
            true,
        )
        .unwrap();

        assert_eq!(output, "hello");
    }

    #[test]
    fn test_render_template_with_loop() {
        let styles = Styles::new().add("item", Style::new().cyan());
        let theme = Theme::from_styles(styles);
        let data = ListData {
            items: vec!["one".into(), "two".into()],
            count: 2,
        };

        let template = r#"{% for item in items %}{{ item | style("item") }}
{% endfor %}"#;

        let output = render_with_color(template, &data, ThemeChoice::from(&theme), false).unwrap();
        assert_eq!(output, "one\ntwo\n");
    }

    #[test]
    fn test_render_mixed_styled_and_plain() {
        let styles = Styles::new().add("count", Style::new().bold());
        let theme = Theme::from_styles(styles);
        let data = ListData {
            items: vec![],
            count: 42,
        };

        let template = r#"Total: {{ count | style("count") }} items"#;
        let output = render_with_color(template, &data, ThemeChoice::from(&theme), false).unwrap();

        assert_eq!(output, "Total: 42 items");
    }

    #[test]
    fn test_render_literal_string_styled() {
        let styles = Styles::new().add("header", Style::new().bold());
        let theme = Theme::from_styles(styles);

        #[derive(Serialize)]
        struct Empty {}

        let output = render_with_color(
            r#"{{ "Header" | style("header") }}"#,
            &Empty {},
            ThemeChoice::from(&theme),
            false,
        )
        .unwrap();

        assert_eq!(output, "Header");
    }

    #[test]
    fn test_renderer_add_and_render() {
        let theme = Theme::new().add("ok", Style::new().green());
        let mut renderer = Renderer::with_color(theme, false);

        renderer
            .add_template("test", r#"{{ message | style("ok") }}"#)
            .unwrap();

        let output = renderer
            .render(
                "test",
                &SimpleData {
                    message: "hi".into(),
                },
            )
            .unwrap();
        assert_eq!(output, "hi");
    }

    #[test]
    fn test_renderer_unknown_template_error() {
        let theme = Theme::new();
        let renderer = Renderer::with_color(theme, false);

        let result = renderer.render(
            "nonexistent",
            &SimpleData {
                message: "x".into(),
            },
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_renderer_multiple_templates() {
        let theme = Theme::new()
            .add("a", Style::new().red())
            .add("b", Style::new().blue());

        let mut renderer = Renderer::with_color(theme, false);
        renderer
            .add_template("tmpl_a", r#"A: {{ message | style("a") }}"#)
            .unwrap();
        renderer
            .add_template("tmpl_b", r#"B: {{ message | style("b") }}"#)
            .unwrap();

        let data = SimpleData {
            message: "test".into(),
        };

        assert_eq!(renderer.render("tmpl_a", &data).unwrap(), "A: test");
        assert_eq!(renderer.render("tmpl_b", &data).unwrap(), "B: test");
    }

    #[test]
    fn test_style_filter_with_nested_data() {
        #[derive(Serialize)]
        struct Item {
            name: String,
            value: i32,
        }

        #[derive(Serialize)]
        struct Container {
            items: Vec<Item>,
        }

        let styles = Styles::new().add("name", Style::new().bold());
        let theme = Theme::from_styles(styles);
        let data = Container {
            items: vec![
                Item {
                    name: "foo".into(),
                    value: 1,
                },
                Item {
                    name: "bar".into(),
                    value: 2,
                },
            ],
        };

        let template = r#"{% for item in items %}{{ item.name | style("name") }}={{ item.value }}
{% endfor %}"#;

        let output = render_with_color(template, &data, ThemeChoice::from(&theme), false).unwrap();
        assert_eq!(output, "foo=1\nbar=2\n");
    }

    #[test]
    fn test_styles_can_be_replaced() {
        let styles = Styles::new()
            .add("x", Style::new().red())
            .add("x", Style::new().green()); // Replace

        // Should only have one style
        assert_eq!(styles.len(), 1);
        assert!(styles.has("x"));
    }

    #[test]
    fn test_empty_template() {
        let styles = Styles::new();
        let theme = Theme::from_styles(styles);

        #[derive(Serialize)]
        struct Empty {}

        let output = render_with_color("", &Empty {}, ThemeChoice::from(&theme), false).unwrap();
        assert_eq!(output, "");
    }

    #[test]
    fn test_template_syntax_error() {
        let styles = Styles::new();
        let theme = Theme::from_styles(styles);

        #[derive(Serialize)]
        struct Empty {}

        let result = render_with_color("{{ unclosed", &Empty {}, ThemeChoice::from(&theme), false);
        assert!(result.is_err());
    }

    #[test]
    fn test_rgb_to_ansi256_grayscale() {
        assert_eq!(rgb_to_ansi256((0, 0, 0)), 16);
        assert_eq!(rgb_to_ansi256((255, 255, 255)), 231);
        let mid = rgb_to_ansi256((128, 128, 128));
        assert!((232..=255).contains(&mid));
    }

    #[test]
    fn test_rgb_to_ansi256_color_cube() {
        assert_eq!(rgb_to_ansi256((255, 0, 0)), 196);
        assert_eq!(rgb_to_ansi256((0, 255, 0)), 46);
        assert_eq!(rgb_to_ansi256((0, 0, 255)), 21);
    }

    #[test]
    fn test_adaptive_theme_uses_detector() {
        console::set_colors_enabled(true);
        let light = Theme::new().add("tone", Style::new().green().force_styling(true));
        let dark = Theme::new().add("tone", Style::new().red().force_styling(true));
        let adaptive = AdaptiveTheme::new(light, dark);
        let data = SimpleData {
            message: "hi".into(),
        };

        set_theme_detector(|| ColorMode::Dark);
        let dark_output = render_with_color(
            r#"{{ message | style("tone") }}"#,
            &data,
            ThemeChoice::Adaptive(&adaptive),
            true,
        )
        .unwrap();
        assert!(dark_output.contains("\x1b[31"));

        set_theme_detector(|| ColorMode::Light);
        let light_output = render_with_color(
            r#"{{ message | style("tone") }}"#,
            &data,
            ThemeChoice::Adaptive(&adaptive),
            true,
        )
        .unwrap();
        assert!(light_output.contains("\x1b[32"));

        // Reset to default for other tests
        set_theme_detector(|| ColorMode::Light);
    }
}
