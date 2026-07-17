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
//! - [`flatten`] — turns the pad *tree* into ordered rows with a depth. MiniJinja has
//!   no clean recursion over a nested structure, and depth is data, not layout: the
//!   template still decides what a depth is worth in spaces.
//! - [`PadRow::section`] — which lifecycle bucket a row's **root** sits in. Templates
//!   drive section breaks off this; it cannot be read off a row's own index, because a
//!   pinned root's children are indexed `Regular` (see [`SectionKind`]).
//! - [`TimeAgo`] — clock arithmetic against `Utc::now()`. It yields a *number and a
//!   unit*, not a sentence; the template composes the label.
//! - [`build_peek`] — delegates to `padzapp::peek`, which owns the preview rules.
//!
//! Everything a template can decide, a template decides.

use super::result::{ModificationResult, PadListResult};
use super::setup::get_grouped_help;
use chrono::{DateTime, Utc};
use minijinja::Value;
use padzapp::api::CmdMessage;
use padzapp::index::{DisplayIndex, DisplayPad, SearchMatch};
use padzapp::model::TodoStatus;
use padzapp::peek::{format_as_peek, PeekResult};
use serde::Serialize;
use standout::context::RenderContext;

/// Minimum terminal width — below this we stop shrinking and let the terminal wrap.
pub const MIN_LINE_WIDTH: usize = 30;
/// Default width when no terminal is detected and COLUMNS is unset (e.g. piped output).
pub const DEFAULT_LINE_WIDTH: usize = 80;

/// The context name `list.jinja` reads its view data from.
pub const LIST_VIEW: &str = "list_view";
/// The context name `modification_result.jinja` reads its view data from.
pub const MODIFICATION_VIEW: &str = "modification_view";
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
///
/// This reads `$COLUMNS` rather than `RenderContext::terminal_width` on purpose: the
/// context field is `None` whenever output is piped, which is exactly the case tests
/// and shell pipelines need to control.
pub fn line_width() -> usize {
    let raw = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .or_else(|| terminal_size::terminal_size().map(|(w, _)| w.0 as usize))
        .unwrap_or(DEFAULT_LINE_WIDTH);
    raw.max(MIN_LINE_WIDTH).saturating_sub(1)
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

/// Template-ready view for a listing.
#[derive(Debug, Clone, Serialize)]
pub struct ListView {
    pub rows: Vec<PadRow>,
    /// The listing was narrowed and matched nothing (vs. the store being empty).
    pub filtered: bool,
    /// Group rows under lifecycle section headers (`--all`).
    pub sections: bool,
    /// Draw todo status icons.
    pub show_status: bool,
    /// `--peek` was asked for. Distinct from a row's own `peek`, which is absent when
    /// a pad has no body: peek *mode* still restyles every title, previewable or not.
    pub peek: bool,
    /// Append the deleted-pads help block.
    pub deleted_help: bool,
    /// The grouped command help, shown when the store is empty. Rendered by clap.
    pub help_text: String,
    pub messages: Vec<CmdMessage>,
}

/// Template-ready view for a modification.
#[derive(Debug, Clone, Serialize)]
pub struct ModificationView {
    /// Past-tense verb for the change ("Created", "Pinned"). The template builds the
    /// sentence around it, including the pluralization.
    pub action: String,
    pub rows: Vec<PadRow>,
    pub show_status: bool,
    pub messages: Vec<CmdMessage>,
}

// =============================================================================
// Context providers (the render-time seam)
// =============================================================================

/// Context provider for the layout width every template reads.
pub fn terminal_provider(_ctx: &RenderContext) -> Value {
    Value::from_serialize(serde_json::json!({ "width": line_width() }))
}

/// Context provider for `list.jinja`.
///
/// Returns `undefined` when the rendered command did not produce a [`PadListResult`].
pub fn list_view_provider(ctx: &RenderContext) -> Value {
    match serde_json::from_value::<PadListResult>(ctx.data.clone()) {
        Ok(result) => Value::from_serialize(build_list_view(&result)),
        Err(_) => Value::UNDEFINED,
    }
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

// =============================================================================
// View builders
// =============================================================================

/// Builds the template-ready view for a listing.
pub fn build_list_view(result: &PadListResult) -> ListView {
    let opts = &result.request;
    let mut rows = Vec::new();
    for dp in &result.pads {
        flatten(dp, 0, SectionKind::of(&dp.index), opts, &mut rows);
    }
    ListView {
        rows,
        filtered: opts.filtered,
        sections: opts.sections,
        show_status: opts.status,
        peek: opts.peek,
        // An empty listing has no deleted pads to explain how to restore, so the
        // help block would be answering a question nobody asked.
        deleted_help: opts.deleted_help && !result.pads.is_empty(),
        // Only paid for when there is nothing else to show.
        help_text: if result.pads.is_empty() && !opts.filtered {
            get_grouped_help()
        } else {
            String::new()
        },
        messages: result.messages.clone(),
    }
}

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
        action: result.action.clone(),
        rows: result
            .pads
            .iter()
            .map(|dp| row(dp, 0, SectionKind::of(&dp.index), &opts))
            .collect(),
        show_status: result.request.status,
        messages: result.messages.clone(),
    }
}

/// Recursively flattens a pad and its children into depth-tagged rows.
///
/// `section` is the *root's* bucket and is carried down unchanged — see [`SectionKind`].
fn flatten(
    dp: &DisplayPad,
    depth: usize,
    section: SectionKind,
    opts: &super::result::ListRequest,
    out: &mut Vec<PadRow>,
) {
    out.push(row(dp, depth, section, opts));
    for child in &dp.children {
        flatten(child, depth + 1, section, opts, out);
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
    let body: String = dp
        .pad
        .content
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n");
    let result = format_as_peek(&body, 3);
    (!result.opening_lines.is_empty()).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::result::{ListRequest, ModificationRequest};
    use padzapp::index::MatchSegment;
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

    fn list(pads: Vec<DisplayPad>, request: ListRequest) -> PadListResult {
        PadListResult {
            pads,
            messages: vec![],
            request,
        }
    }

    // =========================================================================
    // flatten / SectionKind
    // =========================================================================

    /// The whole reason `section` exists rather than reading each row's own index.
    ///
    /// `index_pads` gives a pinned root's children `Regular` indexes, so a template
    /// driving section breaks off the row's own index would split the pinned block
    /// open at its first child. Every row carries its *root's* bucket instead.
    #[test]
    fn a_pinned_roots_children_stay_in_the_pinned_section() {
        let tree = dp(
            pad("root"),
            DisplayIndex::Pinned(1),
            vec![dp(pad("child"), DisplayIndex::Regular(1), vec![])],
        );
        let view = build_list_view(&list(vec![tree], ListRequest::default()));

        assert_eq!(view.rows.len(), 2);
        assert_eq!(view.rows[1].index, DisplayIndex::Regular(1));
        assert_eq!(
            view.rows[1].section,
            SectionKind::Pinned,
            "a child's section is its root's, not its own index's"
        );
    }

    #[test]
    fn flatten_walks_depth_first_and_tags_each_row_with_its_depth() {
        let tree = dp(
            pad("root"),
            DisplayIndex::Regular(1),
            vec![dp(
                pad("child"),
                DisplayIndex::Regular(1),
                vec![dp(pad("grandchild"), DisplayIndex::Regular(1), vec![])],
            )],
        );
        let view = build_list_view(&list(vec![tree], ListRequest::default()));

        let seen: Vec<(&str, usize)> = view
            .rows
            .iter()
            .map(|r| (r.title.as_str(), r.depth))
            .collect();
        assert_eq!(seen, [("root", 0), ("child", 1), ("grandchild", 2)]);
    }

    // =========================================================================
    // Request-driven fields
    // =========================================================================

    #[test]
    fn short_uuid_is_present_only_when_asked_for() {
        let p = pad("p");
        let full = p.metadata.id.to_string();
        let tree = dp(p, DisplayIndex::Regular(1), vec![]);

        let off = build_list_view(&list(vec![tree.clone()], ListRequest::default()));
        assert_eq!(off.rows[0].short_uuid, None);

        let on = build_list_view(&list(
            vec![tree],
            ListRequest {
                uuid: true,
                ..Default::default()
            },
        ));
        assert_eq!(on.rows[0].short_uuid.as_deref(), Some(&full[..8]));
    }

    #[test]
    fn peek_is_absent_without_the_flag_and_when_a_pad_has_no_body() {
        let bodied = dp(pad("has body"), DisplayIndex::Regular(1), vec![]);
        // Empty content normalizes to the title line alone — nothing to preview.
        let bare = dp(
            Pad::new("bare".to_string(), String::new()),
            DisplayIndex::Regular(2),
            vec![],
        );
        let request = ListRequest {
            peek: true,
            ..Default::default()
        };

        let off = build_list_view(&list(vec![bodied.clone()], ListRequest::default()));
        assert!(off.rows[0].peek.is_none(), "no --peek, no preview");

        let on = build_list_view(&list(vec![bodied, bare], request));
        assert!(on.rows[0].peek.is_some());
        assert!(on.rows[1].peek.is_none(), "a bodyless pad previews nothing");
    }

    /// Line 0 is the *title* match, which the pad's own title line already shows;
    /// repeating it as a hit line would print the title twice.
    #[test]
    fn the_title_match_is_not_repeated_as_a_hit_line() {
        let mut tree = dp(pad("p"), DisplayIndex::Regular(1), vec![]);
        tree.matches = Some(vec![
            SearchMatch {
                line_number: 0,
                segments: vec![MatchSegment::Plain("title".into())],
            },
            SearchMatch {
                line_number: 3,
                segments: vec![MatchSegment::Match("body".into())],
            },
        ]);
        let view = build_list_view(&list(vec![tree], ListRequest::default()));

        let lines: Vec<usize> = view.rows[0].matches.iter().map(|m| m.line_number).collect();
        assert_eq!(lines, [3]);
    }

    /// The help block explains how to restore deleted pads. With nothing listed
    /// there is nothing to restore, so it would answer a question nobody asked.
    #[test]
    fn the_deleted_help_block_is_suppressed_on_an_empty_listing() {
        let request = ListRequest {
            deleted_help: true,
            ..Default::default()
        };
        let empty = build_list_view(&list(vec![], request.clone()));
        assert!(!empty.deleted_help);

        let populated = build_list_view(&list(
            vec![dp(pad("p"), DisplayIndex::Deleted(1), vec![])],
            request,
        ));
        assert!(populated.deleted_help);
    }

    /// The grouped help is only rendered when it will actually be shown — it is
    /// clap work, and a populated listing never displays it.
    #[test]
    fn the_grouped_help_is_built_only_for_an_empty_unfiltered_store() {
        assert!(!build_list_view(&list(vec![], ListRequest::default()))
            .help_text
            .is_empty());

        let filtered = ListRequest {
            filtered: true,
            ..Default::default()
        };
        assert!(build_list_view(&list(vec![], filtered))
            .help_text
            .is_empty());

        let populated = build_list_view(&list(
            vec![dp(pad("p"), DisplayIndex::Regular(1), vec![])],
            ListRequest::default(),
        ));
        assert!(populated.help_text.is_empty());
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
            action: "Pinned".to_string(),
            pads: vec![dp(
                pad("root"),
                DisplayIndex::Pinned(1),
                vec![dp(pad("child"), DisplayIndex::Regular(1), vec![])],
            )],
            messages: vec![],
            request: ModificationRequest { status: true },
        };
        let view = build_modification_view(&result);

        assert_eq!(view.action, "Pinned");
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
