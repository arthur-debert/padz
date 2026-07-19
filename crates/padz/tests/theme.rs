//! The default theme is a real artifact, not just a file that happens to parse.
//!
//! # Why these tests exist
//!
//! `styles/default.css` is loaded by `embed_styles!`, which resolves at compile
//! time and cannot fail loudly at runtime the way a missing template does. A theme
//! that silently fails to parse, or that loses a class name a template still emits,
//! degrades quietly: the tag falls through as `[name?]text[/name?]` in the user's
//! terminal. These tests make that a build failure instead.
//!
//! # Why they assert resolved styles rather than ANSI
//!
//! `resolve_styles(mode)` is the same call the renderer makes, so asserting on its
//! output tests the real resolution path — including the `@media` merge onto the
//! base rule, which is where a light/dark migration actually goes wrong. Scraping
//! ANSI would instead pin the palette: every retuned colour would break the suite
//! for no behavioural reason. The one thing worth pinning *is* light≠dark, and
//! that is asserted structurally below.

use standout::{ColorMode, Theme};

const CSS: &str = include_str!("../src/styles/default.css");

fn theme() -> Theme {
    Theme::from_css(CSS).expect("default.css must parse")
}

/// Every semantic class a template, an error path, or clap's help renderer emits.
///
/// A name missing here is a name that renders as `[name?]` in a real terminal, so
/// this list is the theme's contract with `templates/` and must be kept in step
/// with it.
const REQUIRED: &[&str] = &[
    // core
    "title",
    "time",
    "hint",
    // list
    "list-index",
    "list-title",
    "pinned",
    "deleted",
    "deleted-index",
    "deleted-title",
    "status-icon",
    // search
    "highlight",
    "match",
    "line-number",
    // tags
    "tag",
    // semantic message styles used by CLI-owned templates
    "error",
    "warning",
    "success",
    "info",
    // help
    "help-header",
    "help-section",
    "help-command",
    "help-desc",
    "help-usage",
    "help-text",
    // template chrome
    "section-header",
    "empty-message",
    "preview",
    "truncation",
    "separator",
];

#[test]
fn the_theme_defines_every_class_the_templates_emit() {
    for mode in [ColorMode::Light, ColorMode::Dark] {
        let styles = theme().resolve_styles(Some(mode));
        let missing: Vec<_> = REQUIRED.iter().filter(|n| !styles.has(n)).collect();
        assert!(missing.is_empty(), "{mode:?} is missing: {missing:?}");
    }
}

/// The whole point of shipping two variants. If a class resolved identically in
/// both, its `@media` rules are not being applied and the theme is only pretending
/// to adapt — which is exactly how the YAML→CSS migration could have silently
/// half-landed.
#[test]
fn light_and_dark_actually_differ() {
    let (t, mut same) = (theme(), Vec::new());
    let (light, dark) = (
        t.resolve_styles(Some(ColorMode::Light)).to_resolved_map(),
        t.resolve_styles(Some(ColorMode::Dark)).to_resolved_map(),
    );

    for name in REQUIRED {
        // `help-usage` is deliberately scheme-independent: cyan reads on both.
        if *name == "help-usage" {
            continue;
        }
        if format!("{:?}", light[*name]) == format!("{:?}", dark[*name]) {
            same.push(*name);
        }
    }
    assert!(
        same.is_empty(),
        "these resolve the same in light and dark, so their @media rules are dead: {same:?}"
    );
}

/// `.help-usage` carries no `@media` rule, so it must survive as its base colour
/// in both schemes — the case that proves base is a real fallback and not just
/// scaffolding the variants overwrite.
#[test]
fn a_class_without_media_rules_keeps_its_base_in_both_schemes() {
    let t = theme();
    let base = format!(
        "{:?}",
        t.resolve_styles(None).to_resolved_map()["help-usage"]
    );
    for mode in [ColorMode::Light, ColorMode::Dark] {
        assert_eq!(
            format!(
                "{:?}",
                t.resolve_styles(Some(mode)).to_resolved_map()["help-usage"]
            ),
            base,
            "help-usage must not vary by scheme"
        );
    }
}

/// Modifiers live in the base rule and colours in the `@media` rules, which only
/// works because a media rule *merges onto* the base rather than replacing it. If
/// that ever changed upstream, every bold/italic in the theme would vanish in both
/// schemes at once — silently, since the colours would still be right.
#[test]
fn media_rules_merge_onto_the_base_rather_than_replacing_it() {
    let t = theme();
    for mode in [ColorMode::Light, ColorMode::Dark] {
        let styles = t.resolve_styles(Some(mode));
        let title = format!("{:?}", styles.to_resolved_map()["title"]);
        assert!(
            title.contains("Bold"),
            "{mode:?} title lost the bold it inherits from its base rule: {title}"
        );
        let time = format!("{:?}", styles.to_resolved_map()["time"]);
        assert!(
            time.contains("Italic"),
            "{mode:?} time lost the italic it inherits from its base rule: {time}"
        );
    }
}
