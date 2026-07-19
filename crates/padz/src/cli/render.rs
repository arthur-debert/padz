//! # Render-time presentation glue
//!
//! Everything here runs **only** on the human-render path — nothing in this module
//! touches structured output. Templates own wording, glyphs, style names, column widths
//! and indentation; what survives in Rust is the handful of derivations a MiniJinja
//! template cannot do for itself, and each piece earns its place.
//!
//! After the epic that collapsed padz's presentation tiers, this module is exactly three
//! things:
//!
//! ## 1. Two MiniJinja filters (the listing render path)
//!
//! Handlers return core types and `list.jinja` walks the core [`DisplayPad`] tree with a
//! recursive loop (`{% for pad in pads recursive %}` + `loop.depth0`), so depth and
//! section fall out of the tree itself. The only per-value derivation a template cannot
//! do lives in two filters, registered on the engine in [`super::commands`]:
//!
//! - [`timeago_filter`] — clock arithmetic against `Utc::now()` (a template has no
//!   clock). Yields a *number and a unit* ([`TimeAgo`]); the template composes the label.
//! - [`peek_filter`] — delegates to `padzapp::peek::format_as_peek`, which owns the
//!   preview rules.
//!
//! The modification family (`modification_result.jinja`) and the tagging family
//! (`tagging.jinja`, `tag_catalog.jinja`, `tag_registry.jinja`) also render straight from
//! core types, reusing the listing pad line and these filters, so no view mirror
//! survives.
//!
//! ## 2. The terminal-width detector padz installs with Standout
//!
//! [`line_width`]/[`detect_width`] read process state (`$COLUMNS`, the tty) that MiniJinja
//! cannot, and [`super::commands::run`] installs `detect_width` as Standout's terminal-
//! width detector. Standout 7.9.1's built-in detector ignores `$COLUMNS` and returns
//! `None` under a pipe (falling back to a bare 80), which would lose both the `$COLUMNS`
//! control tests and pipelines rely on and the `⏲` under-measure payback — so installing
//! this is what keeps piped and tty output byte-identical. The listing templates then call
//! `tabular([...])` with no `width=` and resolve against it. This is **not** a template
//! width provider; it is the framework-level detector.
//!
//! ## 3. The `terminal` width context provider — a single documented residue
//!
//! [`terminal_provider`] (registered as the [`TERMINAL`] context under
//! `AppBuilder::context_fn` in [`super::commands`], resolved only on the template path)
//! exposes `terminal.width` (== [`line_width`]) to exactly one template: `_match_lines.jinja`.
//! A search-hit line highlights the matched substring in one style and its surroundings in
//! another, then truncates the run to fit — and it must style each segment *after* it is
//! cut, because truncation strips BBCode style tags. That per-segment truncate-then-style
//! math needs the numeric available width, and Standout exposes no template function that
//! returns the raw terminal width (`tabular`/`col('fill')` resolve width internally and
//! never surface it), while a tabular column cannot carry heterogeneous per-segment styles
//! nor truncate without dropping the tags. So this provider survives solely to feed
//! `_match_lines` its width; every other template resolves width through the detector
//! above. When Standout grows a width-returning template function (or `_match_lines` no
//! longer needs per-segment truncation), this provider and [`TERMINAL`] can go.

use chrono::{DateTime, Utc};
use minijinja::Value;
use padzapp::peek::{format_as_peek, PeekResult};
use serde::Serialize;
use standout::context::RenderContext;

/// Minimum terminal width — below this we stop shrinking and let the terminal wrap.
pub const MIN_LINE_WIDTH: usize = 30;
/// Default width when no terminal is detected and COLUMNS is unset (e.g. piped output).
pub const DEFAULT_LINE_WIDTH: usize = 80;

/// The context name every template reads layout width from.
pub const TERMINAL: &str = "terminal";

/// Returns the effective line width for layout.
///
/// Resolution order:
/// 1. `COLUMNS` env var (set by most shells, useful for piped output and tests)
/// 2. Actual terminal width via `terminal_size`
/// 3. `DEFAULT_LINE_WIDTH` (80)
///
/// The result is clamped to at least `MIN_LINE_WIDTH` (30).
///
/// We subtract 1 to compensate for `⏲` (U+23F2) which `unicode-width` measures as 1
/// column but terminals render as 2. Standout's tabular system uses `unicode-width`
/// internally, so without this adjustment every line would overflow by 1 character.
/// `⏲` is Unicode *Narrow*, not East-Asian *ambiguous*, so no ambiguous-width policy
/// pays this back — the `-1` is the only thing that does.
///
/// This reads `$COLUMNS` rather than `RenderContext::terminal_width` on purpose: the
/// context field is `None` whenever output is piped, which is exactly the case tests
/// and shell pipelines need to control.
///
/// Since standout 7.9.1 this is padz's installed terminal-width detector (see
/// [`detect_width`] and [`super::commands::run`]): the framework's `tabular()`/`table()`
/// width cascade resolves against it, so the listing templates call `tabular([...])`
/// with no `width=` and still get this exact width. It also backs the `terminal` context
/// provider ([`terminal_provider`]), which feeds this width to `_match_lines.jinja` — the
/// one template that does manual per-segment truncation a table cannot express.
pub fn line_width() -> usize {
    let raw = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .or_else(|| terminal_size::terminal_size().map(|(w, _)| w.0 as usize))
        .unwrap_or(DEFAULT_LINE_WIDTH);
    raw.max(MIN_LINE_WIDTH).saturating_sub(1)
}

/// padz's terminal-width detector, installed with `set_terminal_width_detector`.
///
/// standout 7.9.1's default detector reads the tty via `terminal_size` and does not
/// consult `$COLUMNS`, so under a pipe it yields `None` and `tabular()` would fall back
/// to a bare 80 — losing both the `$COLUMNS` control tests rely on and the `⏲` payback.
/// Installing this makes the framework width cascade resolve to [`line_width`], which is
/// what keeps piped and tty output byte-identical to before.
pub fn detect_width() -> Option<usize> {
    Some(line_width())
}

// =============================================================================
// timeago derivation
// =============================================================================

/// How long ago something happened, as a number and a unit — never as a sentence.
///
/// The template composes the label (and picks the glyph); this type only does the
/// clock arithmetic, which needs `Utc::now()` and so cannot happen in a template.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct TimeAgo {
    pub value: u64,
    pub unit: char,
}

impl TimeAgo {
    fn since(timestamp: DateTime<Utc>) -> Self {
        let secs = Utc::now()
            .signed_duration_since(timestamp)
            .num_seconds()
            .max(0) as u64;
        let (value, unit) = if secs < 60 {
            (secs, 's')
        } else if secs < 3600 {
            (secs / 60, 'm')
        } else if secs < 86400 {
            (secs / 3600, 'h')
        } else if secs < 86400 * 7 {
            (secs / 86400, 'd')
        } else if secs < 86400 * 30 {
            (secs / (86400 * 7), 'w')
        } else if secs < 86400 * 365 {
            (secs / (86400 * 30), 'M')
        } else {
            (secs / (86400 * 365), 'y')
        };
        Self { value, unit }
    }
}

// =============================================================================
// Context provider (the documented `_match_lines` width residue)
// =============================================================================

/// Context provider exposing `terminal.width` to `_match_lines.jinja`.
///
/// This is the module's single surviving context provider (see the module docs): it
/// resolves only on the template path and feeds [`line_width`] to the one template that
/// does manual per-segment truncation. Every other template resolves width through the
/// installed detector ([`detect_width`]) instead.
pub fn terminal_provider(_ctx: &RenderContext) -> Value {
    Value::from_serialize(serde_json::json!({ "width": line_width() }))
}

// =============================================================================
// peek derivation
// =============================================================================

/// Number of preview lines each `peek` shows at the top and bottom of a body.
const PEEK_LINES: usize = 3;

/// Builds the peek preview for a pad's raw content, or `None` when there is no body.
///
/// The title line is dropped (`.lines().skip(1)`) so the preview shows body only, then
/// `padzapp::peek` decides how many lines to keep and where to elide. An empty result
/// (a pad with nothing under its title) previews nothing.
fn peek_body(content: &str) -> Option<PeekResult> {
    let body: String = content.lines().skip(1).collect::<Vec<_>>().join("\n");
    let result = format_as_peek(&body, PEEK_LINES);
    (!result.opening_lines.is_empty()).then_some(result)
}

// =============================================================================
// MiniJinja filters (the listing render path)
// =============================================================================

/// `timeago` filter: turns a serialized `created_at` timestamp into `{value, unit}`.
///
/// The input is the RFC3339 string serde produces for `DateTime<Utc>`. The template
/// composes the label (`"3m ⏲"`); this only does the clock arithmetic, which needs
/// `Utc::now()` and so cannot live in the template. A value that is not a parseable
/// timestamp renders as `undefined` rather than aborting the whole listing.
pub fn timeago_filter(value: &str) -> Value {
    match DateTime::parse_from_rfc3339(value) {
        Ok(ts) => Value::from_serialize(TimeAgo::since(ts.with_timezone(&Utc))),
        Err(_) => Value::UNDEFINED,
    }
}

/// `peek` filter: previews a pad's body, or `undefined` when it has none.
///
/// Wraps [`peek_body`] so the preview rules stay in `padzapp::peek` and never leak into
/// structured output — the filter only runs on the template path. `undefined` lets the
/// template gate the preview block with a plain `{% if pad.pad.content | peek %}`.
pub fn peek_filter(content: &str) -> Value {
    match peek_body(content) {
        Some(result) => Value::from_serialize(result),
        None => Value::UNDEFINED,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // peek / timeago filters (the listing render path)
    // =========================================================================

    /// The `peek` filter drops the title line and previews the body; a pad whose
    /// content is only its title has nothing to preview and renders `undefined`, which
    /// is what lets the template gate the block with a plain `{% if ... | peek %}`.
    #[test]
    fn peek_filter_previews_a_body_and_is_undefined_without_one() {
        let bodied = peek_filter("has body\n\nbody line");
        assert!(!bodied.is_undefined(), "a pad with a body previews");
        assert_eq!(
            bodied.get_attr("opening_lines").unwrap().as_str(),
            Some("body line")
        );

        // Title line only: nothing under the title to preview.
        assert!(peek_filter("bare").is_undefined());
        assert!(peek_filter("").is_undefined());
    }

    /// The `timeago` filter parses the RFC3339 string serde emits for `created_at` and
    /// returns the `{value, unit}` pair; an unparseable value renders `undefined`
    /// rather than aborting the listing.
    #[test]
    fn timeago_filter_parses_a_timestamp_and_rejects_junk() {
        let long_ago = timeago_filter("2000-01-01T00:00:00Z");
        assert!(!long_ago.is_undefined());
        // Decades ago reads in years.
        assert_eq!(long_ago.get_attr("unit").unwrap().as_str(), Some("y"));

        assert!(timeago_filter("not a timestamp").is_undefined());
    }

    // =========================================================================
    // TimeAgo
    // =========================================================================

    /// `TimeAgo` reports a number and a unit; composing "34s ⏲" is the template's
    /// job. Each boundary picks the largest unit that still yields a whole count.
    #[test]
    fn time_ago_picks_the_largest_whole_unit() {
        let cases = [
            (0i64, 0u64, 's'),
            (59, 59, 's'),
            (60, 1, 'm'),
            (3599, 59, 'm'),
            (3600, 1, 'h'),
            (86_400, 1, 'd'),
            (86_400 * 7, 1, 'w'),
            (86_400 * 30, 1, 'M'),
            (86_400 * 365, 1, 'y'),
        ];
        for (secs, value, unit) in cases {
            let t = TimeAgo::since(Utc::now() - chrono::Duration::seconds(secs));
            assert_eq!((t.value, t.unit), (value, unit), "{secs}s ago");
        }
    }

    /// A clock skewed into the future must not underflow into a huge age.
    #[test]
    fn a_future_timestamp_clamps_to_zero() {
        let t = TimeAgo::since(Utc::now() + chrono::Duration::seconds(600));
        assert_eq!((t.value, t.unit), (0, 's'));
    }

    // =========================================================================
    // line_width
    // =========================================================================

    /// Below `MIN_LINE_WIDTH` we stop shrinking and let the terminal wrap; the
    /// trailing `saturating_sub(1)` pays back ⏲'s under-measured column.
    ///
    /// `#[serial]`: `$COLUMNS` is process-global, so a parallel test reading it
    /// would see this one's value.
    #[test]
    #[serial_test::serial]
    fn line_width_reads_columns_and_clamps_to_the_minimum() {
        let restore = std::env::var("COLUMNS").ok();
        for (columns, expected) in [("10", MIN_LINE_WIDTH - 1), ("100", 99), ("31", 30)] {
            std::env::set_var("COLUMNS", columns);
            assert_eq!(line_width(), expected, "COLUMNS={columns}");
        }
        match restore {
            Some(v) => std::env::set_var("COLUMNS", v),
            None => std::env::remove_var("COLUMNS"),
        }
    }
}
