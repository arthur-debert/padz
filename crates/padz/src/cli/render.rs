//! # Render-time view data
//!
//! This module derives **typed view data** from the mode-independent results that
//! handlers return (see [`super::result`]). It is a render concern only: nothing here
//! runs unless standout has decided to render a human template.
//!
//! ## Architecture
//!
//! ```text
//! handler -> Output::Render(typed result) -> serialize once
//!                                              |-- structured mode: emitted as-is
//!                                              `-- human mode: template + view provider
//! ```
//!
//! Handlers return one value regardless of `--output`. Standout serializes it once and
//! then either emits it directly (json/yaml/xml/csv) or renders a MiniJinja template
//! with it. The providers here are registered as standout **context providers**
//! (`AppBuilder::context_fn`, wired in [`super::commands`]), which standout resolves
//! *only* on the template path. That is the seam that keeps terminal width and
//! relative timestamps out of structured output while still giving templates
//! everything they need — derived from the very same handler value.
//!
//! Providers receive the serialized result as JSON, so each one deserializes it back
//! into its typed result and returns `undefined` if the data is a different command's
//! shape. Templates only read the provider matching their own command, so a
//! non-matching provider is simply unused.
//!
//! ## What is deliberately *not* here
//!
//! No wording, no glyphs, no style names, no column widths, no indentation. Those are
//! presentation policy and live in `templates/` and `styles/default.css`. What survives
//! in Rust is the derivation templates cannot do for themselves, and each piece earns
//! its place:
//!
//! - [`line_width`] — reads process state (`$COLUMNS`, the tty) that MiniJinja cannot.
//! - [`PadRow::section`] — which lifecycle bucket a row's **root** sits in. Templates
//!   drive section breaks off this; it cannot be read off a row's own index, because a
//!   pinned root's children are indexed `Regular` (see [`SectionKind`]).
//! - [`TimeAgo`] — clock arithmetic against `Utc::now()`. It yields a *number and a
//!   unit*, not a sentence; the template composes the label.
//! - [`build_peek`] — delegates to `padzapp::peek`, which owns the preview rules.
//!
//! Everything a template can decide, a template decides.
//!
//! ## Listing renders straight from the core tree
//!
//! The `list`/`search`/`peek` family no longer projects into a flat `*Row` mirror.
//! `list.jinja` walks the core [`DisplayPad`] tree with a MiniJinja recursive loop
//! (`{% for pad in pads recursive %}` + `loop.depth0`), so depth and section fall out
//! of the tree itself. The only Rust that survives on that path is two MiniJinja
//! filters — [`timeago_filter`] (needs `Utc::now()`) and [`peek_filter`] (wraps
//! `padzapp::peek::format_as_peek`) — registered on the template engine in
//! [`super::commands`]. Both are render-path only and never touch structured output.
//!
//! The `PadRow`/`SectionKind`/`TimeAgo`/`build_peek` derivations below are still used
//! by the modification and tagging mirrors (`modification_result.jinja`,
//! `tagging.jinja`), which have not migrated yet; they retire with those families.

use super::result::{
    ModificationActionResult, ModificationNoticeResult, ModificationResult, MutationOutcomeResult,
    MutationStatusResult, TaggingResult, UpdateKindResult,
};
use chrono::{DateTime, Utc};
use minijinja::Value;
use padzapp::index::{DisplayIndex, DisplayPad, SearchMatch};
use padzapp::model::TodoStatus;
use padzapp::peek::{format_as_peek, PeekResult};
use serde::Serialize;
use standout::context::RenderContext;

/// Minimum terminal width — below this we stop shrinking and let the terminal wrap.
pub const MIN_LINE_WIDTH: usize = 30;
/// Default width when no terminal is detected and COLUMNS is unset (e.g. piped output).
pub const DEFAULT_LINE_WIDTH: usize = 80;

/// The context name `modification_result.jinja` reads its view data from.
pub const MODIFICATION_VIEW: &str = "modification_view";
/// The context name `tagging.jinja` reads its view data from.
pub const TAGGING_VIEW: &str = "tagging_view";
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
/// with no `width=` and still get this exact width. It also still backs the `terminal`
/// context provider that the search-hit and modification/tagging templates read
/// directly (those do manual width math a table cannot express).
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
// View types
// =============================================================================

/// Which lifecycle block a row's **root** pad belongs to.
///
/// This is not the same question as "what is this row's own index?". A pinned root is
/// indexed `Pinned`, but its children are indexed `Regular` — so a template that drove
/// section breaks off each row's own index would break the pinned block open at its
/// first child. Every row in a root's subtree carries the *root's* section, which makes
/// "did the section change?" a single comparison against the previous row, at any depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SectionKind {
    Pinned,
    Regular,
    Archived,
    Deleted,
}

impl SectionKind {
    fn of(index: &DisplayIndex) -> Self {
        match index {
            DisplayIndex::Pinned(_) => SectionKind::Pinned,
            DisplayIndex::Regular(_) => SectionKind::Regular,
            DisplayIndex::Archived(_) => SectionKind::Archived,
            DisplayIndex::Deleted(_) => SectionKind::Deleted,
        }
    }
}

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

/// One pad, flattened out of the tree and ready for a template to lay out.
///
/// Every field is data about the pad. Not one of them is a width, a glyph, a style
/// name, or a rendered sentence — `_pad_line.jinja` derives all of those.
#[derive(Debug, Clone, Serialize)]
pub struct PadRow {
    /// This row's own display identifier, e.g. `Pinned(1)` — the template formats it.
    pub index: DisplayIndex,
    /// Depth in the pad tree; 0 for a root. The template decides what a level costs.
    pub depth: usize,
    /// The lifecycle block this row's root sits in. See [`SectionKind`].
    pub section: SectionKind,
    pub title: String,
    /// First 8 characters of the uuid — present only when `--uuid` was asked for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_uuid: Option<String>,
    pub tags: Vec<String>,
    pub status: TodoStatus,
    /// Whether the pad itself is pinned (true in *both* the pinned and regular blocks).
    pub pinned: bool,
    pub time: TimeAgo,
    /// Search hits under this pad; empty unless the listing came from a search.
    pub matches: Vec<SearchMatch>,
    /// Content preview — present only when `--peek` was asked for and there is a body.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peek: Option<PeekResult>,
}

/// Template-ready view for a modification.
#[derive(Debug, Clone, Serialize)]
pub struct ModificationView {
    /// Semantic operation token. The template owns the human verb.
    pub action: ModificationActionResult,
    pub rows: Vec<PadRow>,
    pub show_status: bool,
    /// Presentation-ready projections of semantic core notices.
    pub notices: Vec<ModificationNoticeView>,
    /// Presentation-ready projections of semantic successful outcomes.
    pub outcomes: Vec<MutationOutcomeView>,
}

/// The small amount of display shaping a semantic modification notice needs.
#[derive(Debug, Clone, Serialize)]
pub struct ModificationNoticeView {
    pub kind: &'static str,
    pub index: String,
    pub status: &'static str,
}

/// The small amount of display shaping a semantic update outcome needs.
#[derive(Debug, Clone, Serialize)]
pub struct MutationOutcomeView {
    pub kind: &'static str,
    pub index: String,
    pub title: String,
    pub update_kind: &'static str,
}

/// Template-ready view for per-pad tag assignment and removal.
#[derive(Debug, Clone, Serialize)]
pub struct TaggingView {
    pub action: &'static str,
    pub requested_tags: Vec<String>,
    pub modified_pads: usize,
    pub rows: Vec<PadRow>,
}

// =============================================================================
// Context providers (the render-time seam)
// =============================================================================

/// Context provider for the layout width every template reads.
pub fn terminal_provider(_ctx: &RenderContext) -> Value {
    Value::from_serialize(serde_json::json!({ "width": line_width() }))
}

/// Context provider for `modification_result.jinja`.
///
/// Returns `undefined` when the rendered command did not produce a
/// [`ModificationResult`].
pub fn modification_view_provider(ctx: &RenderContext) -> Value {
    match serde_json::from_value::<ModificationResult>(ctx.data.clone()) {
        Ok(result) => Value::from_serialize(build_modification_view(&result)),
        Err(_) => Value::UNDEFINED,
    }
}

/// Context provider for `tagging.jinja`.
pub fn tagging_view_provider(ctx: &RenderContext) -> Value {
    match serde_json::from_value::<TaggingResult>(ctx.data.clone()) {
        Ok(result) => Value::from_serialize(build_tagging_view(&result)),
        Err(_) => Value::UNDEFINED,
    }
}

// =============================================================================
// View builders
// =============================================================================

/// Builds the template-ready view for a modification result.
///
/// Affected pads are reported as a flat list — a modification names the pads it
/// touched, it does not redraw their subtrees — so every row here is at depth 0.
pub fn build_modification_view(result: &ModificationResult) -> ModificationView {
    let opts = super::result::ListRequest {
        status: result.request.status,
        ..Default::default()
    };
    ModificationView {
        action: result.action,
        rows: result
            .pads
            .iter()
            .map(|dp| row(dp, 0, SectionKind::of(&dp.index), &opts))
            .collect(),
        show_status: result.request.status,
        notices: result.notices.iter().map(modification_notice).collect(),
        outcomes: result
            .outcomes
            .iter()
            .filter_map(mutation_outcome)
            .collect(),
    }
}

/// Builds the template-ready view for a tag assignment or removal.
pub fn build_tagging_view(result: &TaggingResult) -> TaggingView {
    let (action, requested_tags, modified_pads, pads) = match result {
        TaggingResult::Assigned {
            requested_tags,
            modified_pads,
            pads,
        } => ("assigned", requested_tags, *modified_pads, pads),
        TaggingResult::AllAlreadyPresent {
            requested_tags,
            modified_pads,
            pads,
        } => ("all_already_present", requested_tags, *modified_pads, pads),
        TaggingResult::Removed {
            requested_tags,
            modified_pads,
            pads,
        } => ("removed", requested_tags, *modified_pads, pads),
        TaggingResult::NonePresent {
            requested_tags,
            modified_pads,
            pads,
        } => ("none_present", requested_tags, *modified_pads, pads),
    };
    let options = super::result::ListRequest::default();

    TaggingView {
        action,
        requested_tags: requested_tags.clone(),
        modified_pads,
        rows: pads
            .iter()
            .map(|pad| row(pad, 0, SectionKind::of(&pad.index), &options))
            .collect(),
    }
}

fn modification_notice(notice: &ModificationNoticeResult) -> ModificationNoticeView {
    let (kind, path, status) = match notice {
        ModificationNoticeResult::AlreadyPinned { path } => ("already_pinned", path, ""),
        ModificationNoticeResult::AlreadyUnpinned { path } => ("already_unpinned", path, ""),
        ModificationNoticeResult::AlreadyAtDestination { path } => {
            ("already_at_destination", path, "")
        }
        ModificationNoticeResult::AlreadyInStatus { path, status } => (
            "already_in_status",
            path,
            match status {
                MutationStatusResult::Planned => "planned",
                MutationStatusResult::InProgress => "in progress",
                MutationStatusResult::Done => "done",
            },
        ),
        ModificationNoticeResult::NoCompletedPads => {
            return ModificationNoticeView {
                kind: "no_completed_pads",
                index: String::new(),
                status: "",
            };
        }
    };
    ModificationNoticeView {
        kind,
        index: path
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("."),
        status,
    }
}

fn mutation_outcome(outcome: &MutationOutcomeResult) -> Option<MutationOutcomeView> {
    match outcome {
        MutationOutcomeResult::Updated {
            path,
            title,
            update_kind,
        } => Some(MutationOutcomeView {
            kind: "updated",
            index: path
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("."),
            title: title.clone(),
            update_kind: match update_kind {
                UpdateKindResult::Structured => "structured",
                UpdateKindResult::Content => "content",
                UpdateKindResult::Refresh => "refresh",
            },
        }),
        MutationOutcomeResult::StatusChanged { .. } => None,
    }
}

/// Builds one row from a pad.
fn row(
    dp: &DisplayPad,
    depth: usize,
    section: SectionKind,
    opts: &super::result::ListRequest,
) -> PadRow {
    let meta = &dp.pad.metadata;
    PadRow {
        index: dp.index.clone(),
        depth,
        section,
        title: meta.title.clone(),
        short_uuid: opts.uuid.then(|| meta.id.to_string()[..8].to_string()),
        tags: meta.tags.clone(),
        status: meta.status,
        pinned: meta.is_pinned,
        time: TimeAgo::since(meta.created_at),
        // Line 0 is the title match, which the pad's own title line already shows.
        matches: dp
            .matches
            .iter()
            .flatten()
            .filter(|m| m.line_number != 0)
            .cloned()
            .collect(),
        peek: opts.peek.then(|| build_peek(dp)).flatten(),
    }
}

/// Builds the peek preview for a pad, or `None` when it has no body to preview.
///
/// The preview rules (how many lines, where to elide) belong to `padzapp::peek`; this
/// only feeds it the body and drops an empty result.
fn build_peek(dp: &DisplayPad) -> Option<PeekResult> {
    peek_body(&dp.pad.content)
}

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
    use crate::cli::result::ModificationRequest;
    use padzapp::model::Pad;

    fn pad(title: &str) -> Pad {
        Pad::new(title.to_string(), format!("{title}\n\nbody line"))
    }

    fn dp(pad: Pad, index: DisplayIndex, children: Vec<DisplayPad>) -> DisplayPad {
        DisplayPad {
            pad,
            index,
            matches: None,
            children,
        }
    }

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
    // Modification view
    // =========================================================================

    /// A modification names the pads it touched; it does not redraw their subtrees.
    #[test]
    fn a_modification_reports_affected_pads_flat() {
        let result = ModificationResult {
            action: ModificationActionResult::Pin,
            pads: vec![dp(
                pad("root"),
                DisplayIndex::Pinned(1),
                vec![dp(pad("child"), DisplayIndex::Regular(1), vec![])],
            )],
            notices: vec![],
            outcomes: vec![],
            request: ModificationRequest { status: true },
        };
        let view = build_modification_view(&result);

        assert_eq!(view.action, ModificationActionResult::Pin);
        assert_eq!(view.rows.len(), 1, "children are not redrawn");
        assert_eq!(view.rows[0].depth, 0);
        assert!(view.show_status);
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
