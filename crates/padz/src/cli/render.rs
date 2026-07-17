//! # Rendering Module
//!
//! This module derives **template-ready view data** from the typed, mode-independent
//! results that handlers return (see [`super::result`]). It is a render concern only:
//! nothing here runs unless standout has decided to render a human template.
//!
//! ## Architecture
//!
//! ```text
//! handler -> Output::Render(typed result) -> serialize once
//!                                              |-- structured mode: emitted as-is
//!                                              `-- human mode: template + view builder
//! ```
//!
//! Handlers return one value regardless of `--output`. Standout serializes it once and
//! then either emits it directly (json/yaml/xml/csv) or renders a MiniJinja template
//! with it. The view builders here are registered as standout **context providers**
//! (`AppBuilder::context_fn`, wired in [`super::commands`]), which standout resolves
//! *only* on the template path. That is the seam that keeps column widths, glyphs, and
//! relative timestamps out of structured output while still giving templates
//! everything they need — derived from the very same handler value.
//!
//! Providers receive the serialized result as JSON, so each one deserializes it back
//! into its typed result and returns `undefined` if the data is a different command's
//! shape. Templates only read the provider matching their own command, so a
//! non-matching provider is simply unused.
//!
//! ## Table Layout
//!
//! The list view uses standout's `tabular()` filter for declarative column layout.
//! Each row has:
//! - `left_pin` (2 chars): Pin marker for pinned pads (both sections) or empty
//! - `status_icon` (2 chars): Todo status indicator
//! - `index` (4 chars): Display index (p1., 1., d1.)
//! - `title` (fill): Pad title, truncated to fit
//! - `time_ago` (14 chars, right-aligned): Relative timestamp
//!
//! Column widths are defined as constants and the title width is calculated per-row
//! based on the variable prefix width (which depends on section type and nesting
//! depth). Each row carries its own column widths so that `_pad_line.jinja` is
//! self-contained and can be shared by the list and modification templates.

use super::result::{ModificationResult, PadListResult};
use super::setup::get_grouped_help;
use chrono::{DateTime, Utc};
use minijinja::Value;
use padzapp::api::{CmdMessage, MessageLevel, TodoStatus};
use padzapp::index::{DisplayIndex, DisplayPad};
use padzapp::peek::format_as_peek;
use standout::context::RenderContext;
use standout::truncate_to_width;

/// Minimum terminal width — below this we stop shrinking and let the terminal wrap.
pub const MIN_LINE_WIDTH: usize = 30;
/// Default width when no terminal is detected and COLUMNS is unset (e.g. piped output).
pub const DEFAULT_LINE_WIDTH: usize = 80;
pub const PIN_MARKER: &str = "⚲";

/// Returns the effective line width for layout.
///
/// Resolution order:
/// 1. `COLUMNS` env var (set by most shells, useful for piped output and tests)
/// 2. Actual terminal width via `terminal_size`
/// 3. `DEFAULT_LINE_WIDTH` (80)
///
/// The result is clamped to at least `MIN_LINE_WIDTH` (30).
///
/// We subtract 1 to compensate for `⏲` (U+23F2) which `unicode-width` measures as 1 column
/// but terminals render as 2. Standout's tabular system uses `unicode-width` internally, so
/// without this adjustment every line would overflow the terminal by 1 character.
pub fn line_width() -> usize {
    let raw = std::env::var("COLUMNS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .or_else(|| terminal_size::terminal_size().map(|(w, _)| w.0 as usize))
        .unwrap_or(DEFAULT_LINE_WIDTH);
    raw.max(MIN_LINE_WIDTH).saturating_sub(1)
}

/// Column widths for list layout (used by standout's `tabular()` filter)
pub const COL_LEFT_PIN: usize = 2; // Pin marker or empty ("⚲ " or "  ")
pub const COL_STATUS: usize = 2; // Status icon + space
pub const COL_INDEX: usize = 4; // "p1.", " 1.", "d1."
pub const COL_TIME: usize = 5; // Compact timestamp ("34s ⏲" per unicode-width)

/// Status indicators for todo status
pub const STATUS_PLANNED: &str = "⚪︎";
pub const STATUS_IN_PROGRESS: &str = "☉︎︎";
pub const STATUS_DONE: &str = "⚫︎";

/// The context name `list.jinja` reads its view data from.
pub const LIST_VIEW: &str = "list_view";
/// The context name `modification_result.jinja` reads its view data from.
pub const MODIFICATION_VIEW: &str = "modification_view";

// =============================================================================
// Context providers (the render-time seam)
// =============================================================================

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

/// Builds the template-ready view for a modification result.
///
/// Produces the start message ("Created 1 pad..."), one self-contained row per
/// affected pad, and the trailing messages.
pub fn build_modification_view(result: &ModificationResult) -> serde_json::Value {
    use serde_json::json;

    let show_status = result.request.status;
    let col_status = if show_status { COL_STATUS } else { 0 };
    let width = line_width();

    let count = result.pads.len();
    let start_message = if count == 0 {
        String::new()
    } else {
        let pad_word = if count == 1 { "pad" } else { "pads" };
        format!("{} {} {}...", result.action, count, pad_word)
    };

    let pad_lines: Vec<serde_json::Value> = result
        .pads
        .iter()
        .map(|dp| {
            let fixed_columns = COL_LEFT_PIN + col_status + COL_INDEX + COL_TIME;
            let mut row = base_pad_row(
                dp,
                RowLayout {
                    indent_width: 0,
                    title_width: width.saturating_sub(fixed_columns),
                    show_status,
                    col_status,
                    line_width: width,
                    is_peek: false,
                },
            );
            if dp.pad.metadata.is_pinned {
                row["left_pin"] = json!(PIN_MARKER);
            }
            row["is_pinned_section"] = json!(matches!(dp.index, DisplayIndex::Pinned(_)));
            row["is_deleted"] = json!(matches!(dp.index, DisplayIndex::Deleted(_)));
            row
        })
        .collect();

    json!({
        "start_message": start_message,
        "pads": pad_lines,
        "trailing_messages": convert_messages_to_json(&result.messages),
    })
}

/// Builds the template-ready view for a listing.
///
/// Flattens the pad tree into indented rows, carries per-row column widths, and
/// derives the empty-state and section-header structure.
pub fn build_list_view(result: &PadListResult) -> serde_json::Value {
    use serde_json::json;

    let opts = &result.request;
    let show_status = opts.status;
    let col_status = if show_status { COL_STATUS } else { 0 };
    let width = line_width();
    let trailing_data = convert_messages_to_json(&result.messages);

    if result.pads.is_empty() {
        if opts.filtered {
            return json!({
                "pads": [],
                "empty_filtered": true,
                "trailing_messages": trailing_data,
            });
        }
        return json!({
            "pads": [],
            "empty": true,
            "help_text": get_grouped_help(),
            "deleted_help": false,
            "trailing_messages": trailing_data,
        });
    }

    let mut pad_lines: Vec<serde_json::Value> = Vec::new();
    let mut last_was_pinned = false;
    let mut entered_archived = false;
    let mut entered_deleted = false;

    for dp in &result.pads {
        let is_pinned_section = matches!(dp.index, DisplayIndex::Pinned(_));
        let is_archived_section = matches!(dp.index, DisplayIndex::Archived(_));
        let is_deleted_section = matches!(dp.index, DisplayIndex::Deleted(_));

        // Separator between pinned and regular roots
        if last_was_pinned && !is_pinned_section {
            pad_lines.push(separator_row());
        }
        last_was_pinned = is_pinned_section;

        // Section headers for --all mode
        if opts.sections {
            if is_archived_section && !entered_archived {
                entered_archived = true;
                pad_lines.push(json!({ "is_separator": true, "is_section_header": false }));
                pad_lines
                    .push(json!({ "is_section_header": true, "section_title": "Archived Pads" }));
            }
            if is_deleted_section && !entered_deleted {
                entered_deleted = true;
                pad_lines.push(json!({ "is_separator": true, "is_section_header": false }));
                pad_lines
                    .push(json!({ "is_section_header": true, "section_title": "Deleted Pads" }));
            }
        }

        push_pad_row(
            dp,
            &mut pad_lines,
            0,
            is_pinned_section,
            is_deleted_section,
            opts,
            show_status,
            col_status,
            width,
        );
    }

    json!({
        "pads": pad_lines,
        "empty": false,
        "help_text": "",
        "deleted_help": opts.deleted_help,
        "trailing_messages": trailing_data,
    })
}

/// Layout inputs for one rendered pad row.
struct RowLayout {
    indent_width: usize,
    title_width: usize,
    show_status: bool,
    col_status: usize,
    line_width: usize,
    is_peek: bool,
}

/// Builds the fields every pad row shares.
///
/// Each row is self-contained — it carries its own column widths and line width — so
/// that `_pad_line.jinja` needs nothing but the row itself and can be included from
/// both the list and modification templates.
///
/// The row's nesting indent ships in two forms: `indent` (the literal spaces every
/// partial prefixes its lines with) and `indent_width` (the same value as a number,
/// which `_peek_content.jinja` adds to the `indent()` filter so a peek block's
/// continuation lines line up with its own first line).
fn base_pad_row(dp: &DisplayPad, layout: RowLayout) -> serde_json::Value {
    use serde_json::json;

    let local_idx_str = match &dp.index {
        DisplayIndex::Pinned(n) => format!("p{}", n),
        DisplayIndex::Regular(n) => format!("{:2}", n),
        DisplayIndex::Archived(n) => format!("ar{}", n),
        DisplayIndex::Deleted(n) => format!("d{}", n),
    };

    let status_icon = if layout.show_status {
        match dp.pad.metadata.status {
            TodoStatus::Planned => STATUS_PLANNED,
            TodoStatus::InProgress => STATUS_IN_PROGRESS,
            TodoStatus::Done => STATUS_DONE,
        }
    } else {
        ""
    };

    json!({
        "indent": " ".repeat(layout.indent_width),
        "indent_width": layout.indent_width,
        "left_pin": "",
        "status_icon": status_icon,
        "index": format!("{}.", local_idx_str),
        "title": dp.pad.metadata.title,
        "title_width": layout.title_width,
        "tags": dp.pad.metadata.tags,
        "tags_display": format_tags_display(&dp.pad.metadata.tags),
        "time_ago": format_time_ago(dp.pad.metadata.created_at),
        "is_pinned_section": false,
        "is_deleted": false,
        "is_separator": false,
        "is_peek": layout.is_peek,
        "matches": [],
        "more_matches_count": 0,
        "peek": serde_json::Value::Null,
        "line_width": layout.line_width,
        "cols": {
            "left_pin": COL_LEFT_PIN,
            "status": layout.col_status,
            "index": COL_INDEX,
            "time": COL_TIME,
        },
    })
}

/// A blank row separating the pinned block from the regular block.
fn separator_row() -> serde_json::Value {
    serde_json::json!({
        "is_separator": true,
        "is_section_header": false,
    })
}

/// Recursively flattens a pad and its children into indented rows.
#[allow(clippy::too_many_arguments)]
fn push_pad_row(
    dp: &DisplayPad,
    pad_lines: &mut Vec<serde_json::Value>,
    depth: usize,
    is_pinned_section: bool,
    is_deleted_root: bool,
    opts: &super::result::ListRequest,
    show_status: bool,
    col_status: usize,
    width: usize,
) {
    let is_deleted = matches!(dp.index, DisplayIndex::Deleted(_));
    let indent_width = depth * 2;
    let fixed_columns = COL_LEFT_PIN + col_status + COL_INDEX + COL_TIME;

    let mut row = base_pad_row(
        dp,
        RowLayout {
            indent_width,
            title_width: width.saturating_sub(fixed_columns + indent_width),
            show_status,
            col_status,
            line_width: width,
            is_peek: opts.peek,
        },
    );

    if dp.pad.metadata.is_pinned && depth == 0 {
        row["left_pin"] = serde_json::json!(PIN_MARKER);
    }
    row["is_pinned_section"] = serde_json::json!(is_pinned_section && depth == 0);
    row["is_deleted"] = serde_json::json!(is_deleted || is_deleted_root);
    row["matches"] = serde_json::json!(build_match_lines(dp, indent_width, col_status, width));

    if opts.peek {
        row["peek"] = build_peek(dp);
    }

    if opts.uuid {
        let short_uuid = &dp.pad.metadata.id.to_string()[..8];
        row["title"] = serde_json::json!(format!("({}) {}", short_uuid, dp.pad.metadata.title));
    }

    pad_lines.push(row);

    for child in &dp.children {
        push_pad_row(
            child,
            pad_lines,
            depth + 1,
            is_pinned_section,
            is_deleted_root,
            opts,
            show_status,
            col_status,
            width,
        );
    }
}

/// Builds the styled, width-truncated search-match lines shown under a pad.
fn build_match_lines(
    dp: &DisplayPad,
    indent_width: usize,
    col_status: usize,
    width: usize,
) -> Vec<serde_json::Value> {
    let mut match_lines: Vec<serde_json::Value> = Vec::new();
    let Some(matches) = &dp.matches else {
        return match_lines;
    };

    for m in matches {
        if m.line_number == 0 {
            continue;
        }
        let segments: Vec<serde_json::Value> = m
            .segments
            .iter()
            .map(|s| {
                let (text, style) = match s {
                    padzapp::index::MatchSegment::Plain(t) => (t.clone(), "info"),
                    padzapp::index::MatchSegment::Match(t) => (t.clone(), "match"),
                };
                serde_json::json!({ "text": text, "style": style })
            })
            .collect();

        let match_indent = indent_width + COL_LEFT_PIN + col_status + COL_INDEX;
        let match_available = width.saturating_sub(COL_TIME + match_indent);

        match_lines.push(serde_json::json!({
            "line_number": format!("{:02}", m.line_number),
            "segments": truncate_match_segments_to_json(&segments, match_available),
        }));
    }
    match_lines
}

/// Builds the peek preview for a pad, or `null` when it has no body to preview.
fn build_peek(dp: &DisplayPad) -> serde_json::Value {
    let body_lines: Vec<&str> = dp.pad.content.lines().skip(1).collect();
    let body = body_lines.join("\n");
    let result = format_as_peek(&body, 3);
    if result.opening_lines.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::to_value(&result).unwrap_or(serde_json::Value::Null)
    }
}

/// Helper to convert CmdMessages to JSON values for templates
fn convert_messages_to_json(messages: &[CmdMessage]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|msg| {
            let style = match msg.level {
                MessageLevel::Info => "info",
                MessageLevel::Success => "success",
                MessageLevel::Warning => "warning",
                MessageLevel::Error => "error",
            };
            serde_json::json!({
                "content": msg.content,
                "style": style,
            })
        })
        .collect()
}

/// Helper to truncate match segments to available width (returns JSON values)
fn truncate_match_segments_to_json(
    segments: &[serde_json::Value],
    max_width: usize,
) -> Vec<serde_json::Value> {
    use unicode_width::UnicodeWidthStr;

    let mut result = Vec::new();
    let mut current_width = 0;

    for seg in segments {
        let text = seg.get("text").and_then(|v| v.as_str()).unwrap_or("");
        let style = seg
            .get("style")
            .and_then(|v| v.as_str())
            .unwrap_or("info")
            .to_string();
        let w = text.width();
        if current_width + w <= max_width {
            result.push(seg.clone());
            current_width += w;
        } else {
            let remaining = max_width.saturating_sub(current_width);
            let truncated = truncate_to_width(text, remaining);
            result.push(serde_json::json!({
                "text": truncated,
                "style": style,
            }));
            return result;
        }
    }
    result
}

fn format_tags_display(tags: &[String]) -> String {
    tags.iter()
        .map(|t| format!("\u{300c}[tag]{}[/tag]\u{300d}", t.trim()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_time_ago(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let secs = now.signed_duration_since(timestamp).num_seconds().max(0) as u64;

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

    format!("{:2}{} ⏲", value, unit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::result::{ListRequest, ModificationRequest};
    use padzapp::model::Pad;

    fn make_pad(title: &str, pinned: bool) -> Pad {
        let mut p = Pad::new(title.to_string(), "some content".to_string());
        p.metadata.is_pinned = pinned;
        p
    }

    #[test]
    fn test_time_col_matches_unicode_width() {
        use unicode_width::UnicodeWidthStr;

        // COL_TIME must match what unicode-width reports for the time format.
        // Note: ⏲ (U+23F2) is 1 col per unicode-width but 2 in terminals.
        // We compensate via line_width()'s saturating_sub(1).
        let time_sample = format!("{:2}{} ⏲", 34, 's');
        let time_width = time_sample.width();
        assert_eq!(
            time_width, COL_TIME,
            "time '{}' has display width {}, COL_TIME is {}",
            time_sample, time_width, COL_TIME
        );
    }

    fn make_display_pad(pad: Pad, index: DisplayIndex) -> DisplayPad {
        DisplayPad {
            pad,
            index,
            matches: None,
            children: vec![],
        }
    }

    fn list_result(pads: Vec<DisplayPad>, request: ListRequest) -> PadListResult {
        PadListResult {
            pads,
            messages: vec![],
            request,
        }
    }

    /// Todos-mode listing: status icons on, nothing else requested.
    fn todos_request() -> ListRequest {
        ListRequest {
            status: true,
            ..Default::default()
        }
    }

    /// Notes-mode listing: no status icons.
    fn notes_request() -> ListRequest {
        ListRequest::default()
    }

    fn modification_result(
        action: &str,
        pads: Vec<DisplayPad>,
        status: bool,
    ) -> ModificationResult {
        ModificationResult {
            action: action.to_string(),
            pads,
            messages: vec![],
            request: ModificationRequest { status },
        }
    }

    fn row_col_status(row: &serde_json::Value) -> u64 {
        row.get("cols")
            .and_then(|c| c.get("status"))
            .and_then(|v| v.as_u64())
            .unwrap()
    }

    #[test]
    fn test_build_list_empty() {
        let data = build_list_view(&list_result(vec![], todos_request()));
        assert!(data.get("empty").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(data
            .get("help_text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("create"));
    }

    #[test]
    fn test_build_list_empty_filtered() {
        let request = ListRequest {
            filtered: true,
            ..todos_request()
        };
        let data = build_list_view(&list_result(vec![], request));
        assert_eq!(
            data.get("empty_filtered").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_build_list_single_regular_pad() {
        let dp = make_display_pad(make_pad("Test Note", false), DisplayIndex::Regular(1));
        let data = build_list_view(&list_result(vec![dp], todos_request()));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(pads.len(), 1);

        let pad_data = &pads[0];
        assert_eq!(
            pad_data.get("title").and_then(|v| v.as_str()),
            Some("Test Note")
        );
        assert_eq!(pad_data.get("index").and_then(|v| v.as_str()), Some(" 1."));
        assert_eq!(
            pad_data.get("status_icon").and_then(|v| v.as_str()),
            Some(STATUS_PLANNED)
        );
    }

    #[test]
    fn test_build_list_pinned_pad() {
        let dp = make_display_pad(make_pad("Pinned Note", true), DisplayIndex::Pinned(1));
        let data = build_list_view(&list_result(vec![dp], todos_request()));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        let pad_data = &pads[0];

        assert_eq!(pad_data.get("index").and_then(|v| v.as_str()), Some("p1."));
        assert_eq!(
            pad_data.get("left_pin").and_then(|v| v.as_str()),
            Some(PIN_MARKER)
        );
        assert_eq!(
            pad_data.get("is_pinned_section").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_build_list_deleted_pad() {
        let dp = make_display_pad(make_pad("Deleted Note", false), DisplayIndex::Deleted(1));
        let request = ListRequest {
            deleted_help: true,
            ..todos_request()
        };
        let data = build_list_view(&list_result(vec![dp], request));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        let pad_data = &pads[0];

        assert_eq!(pad_data.get("index").and_then(|v| v.as_str()), Some("d1."));
        assert_eq!(
            pad_data.get("is_deleted").and_then(|v| v.as_bool()),
            Some(true)
        );
        assert_eq!(
            data.get("deleted_help").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_build_list_mixed_pinned_and_regular() {
        let pads = vec![
            make_display_pad(make_pad("Pinned", true), DisplayIndex::Pinned(1)),
            make_display_pad(make_pad("Regular", false), DisplayIndex::Regular(1)),
        ];

        let data = build_list_view(&list_result(pads, todos_request()));
        let pad_list = data.get("pads").and_then(|v| v.as_array()).unwrap();
        // Should have: pinned pad, separator, regular pad
        assert_eq!(pad_list.len(), 3);

        // Middle item should be separator
        assert_eq!(
            pad_list[1].get("is_separator").and_then(|v| v.as_bool()),
            Some(true)
        );
    }

    #[test]
    fn test_build_list_pinned_in_regular_section_shows_left_pin() {
        // A pinned pad displayed in regular section should show left_pin
        let dp = make_display_pad(make_pad("Pinned Note", true), DisplayIndex::Regular(1));
        let data = build_list_view(&list_result(vec![dp], todos_request()));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(
            pads[0].get("left_pin").and_then(|v| v.as_str()),
            Some(PIN_MARKER)
        );
    }

    #[test]
    fn test_build_list_with_messages() {
        let dp = make_display_pad(make_pad("Test", false), DisplayIndex::Regular(1));
        let result = PadListResult {
            pads: vec![dp],
            messages: vec![CmdMessage::success("Operation completed")],
            request: todos_request(),
        };

        let data = build_list_view(&result);
        let trailing = data
            .get("trailing_messages")
            .and_then(|v| v.as_array())
            .unwrap();
        assert_eq!(trailing.len(), 1);
        assert_eq!(
            trailing[0].get("content").and_then(|v| v.as_str()),
            Some("Operation completed")
        );
        assert_eq!(
            trailing[0].get("style").and_then(|v| v.as_str()),
            Some("success")
        );
    }

    #[test]
    fn test_build_list_uuid_prefixes_title() {
        let pad = make_pad("Test", false);
        let short = pad.metadata.id.to_string()[..8].to_string();
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));
        let request = ListRequest {
            uuid: true,
            ..todos_request()
        };

        let data = build_list_view(&list_result(vec![dp], request));
        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(
            pads[0].get("title").and_then(|v| v.as_str()),
            Some(format!("({}) Test", short).as_str())
        );
    }

    #[test]
    fn test_build_modification_result() {
        let dp = make_display_pad(make_pad("Test", false), DisplayIndex::Regular(1));
        let data = build_modification_view(&modification_result("Created", vec![dp], true));

        assert_eq!(
            data.get("start_message").and_then(|v| v.as_str()),
            Some("Created 1 pad...")
        );
    }

    #[test]
    fn test_build_modification_result_shows_pin_marker() {
        // A pinned pad keeps its marker when reported by a modification command.
        let dp = make_display_pad(make_pad("Pinned", true), DisplayIndex::Pinned(2));
        let data = build_modification_view(&modification_result("Pinned", vec![dp], false));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(
            pads[0].get("left_pin").and_then(|v| v.as_str()),
            Some(PIN_MARKER)
        );
    }

    #[test]
    fn test_build_modification_result_unpinned_has_no_marker() {
        let dp = make_display_pad(make_pad("Plain", false), DisplayIndex::Regular(1));
        let data = build_modification_view(&modification_result("Created", vec![dp], false));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(pads[0].get("left_pin").and_then(|v| v.as_str()), Some(""));
    }

    #[test]
    fn test_build_modification_result_pluralizes() {
        let pads = vec![
            make_display_pad(make_pad("A", false), DisplayIndex::Regular(1)),
            make_display_pad(make_pad("B", false), DisplayIndex::Regular(2)),
        ];
        let data = build_modification_view(&modification_result("Deleted", pads, false));

        assert_eq!(
            data.get("start_message").and_then(|v| v.as_str()),
            Some("Deleted 2 pads...")
        );
    }

    #[test]
    fn test_build_modification_result_empty_has_no_start_message() {
        let data = build_modification_view(&modification_result("Deleted", vec![], false));
        assert_eq!(data.get("start_message").and_then(|v| v.as_str()), Some(""));
    }

    #[test]
    fn test_format_time_ago_compact() {
        use chrono::Duration;

        let now = Utc::now();

        let test_cases = [
            (Duration::seconds(5), " 5s ⏲"),
            (Duration::seconds(34), "34s ⏲"),
            (Duration::minutes(3), " 3m ⏲"),
            (Duration::minutes(59), "59m ⏲"),
            (Duration::hours(2), " 2h ⏲"),
            (Duration::hours(23), "23h ⏲"),
            (Duration::days(3), " 3d ⏲"),
            (Duration::days(6), " 6d ⏲"),
            (Duration::weeks(2), " 2w ⏲"),
            (Duration::days(45), " 1M ⏲"),
            (Duration::days(400), " 1y ⏲"),
        ];

        for (duration, expected) in test_cases {
            let timestamp = now - duration;
            let formatted = format_time_ago(timestamp);
            assert_eq!(
                formatted, expected,
                "Duration {:?} should format as '{}'",
                duration, expected
            );
        }
    }

    #[test]
    fn test_notes_mode_hides_status_icon() {
        let dp = make_display_pad(make_pad("Test Note", false), DisplayIndex::Regular(1));
        let data = build_list_view(&list_result(vec![dp], notes_request()));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(
            pads[0].get("status_icon").and_then(|v| v.as_str()),
            Some("")
        );
        assert_eq!(row_col_status(&pads[0]), 0);
    }

    #[test]
    fn test_todos_mode_shows_status_icon() {
        let dp = make_display_pad(make_pad("Test Note", false), DisplayIndex::Regular(1));
        let data = build_list_view(&list_result(vec![dp], todos_request()));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(
            pads[0].get("status_icon").and_then(|v| v.as_str()),
            Some(STATUS_PLANNED)
        );
        assert_eq!(row_col_status(&pads[0]), COL_STATUS as u64);
    }

    #[test]
    fn test_force_show_status_in_modification_result() {
        let dp = make_display_pad(make_pad("Test Note", false), DisplayIndex::Regular(1));
        // A status-changing command in notes mode still shows status icons.
        let data = build_modification_view(&modification_result("Completed", vec![dp], true));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(
            pads[0].get("status_icon").and_then(|v| v.as_str()),
            Some(STATUS_PLANNED)
        );
        assert_eq!(row_col_status(&pads[0]), COL_STATUS as u64);
    }

    #[test]
    fn test_notes_mode_gives_more_title_width() {
        let pad = make_pad("Test Note", false);
        let dp = make_display_pad(pad.clone(), DisplayIndex::Regular(1));
        let dp2 = make_display_pad(pad, DisplayIndex::Regular(1));

        let notes_data = build_list_view(&list_result(vec![dp], notes_request()));
        let todos_data = build_list_view(&list_result(vec![dp2], todos_request()));

        let notes_width = notes_data.get("pads").and_then(|v| v.as_array()).unwrap()[0]
            .get("title_width")
            .and_then(|v| v.as_u64())
            .unwrap();
        let todos_width = todos_data.get("pads").and_then(|v| v.as_array()).unwrap()[0]
            .get("title_width")
            .and_then(|v| v.as_u64())
            .unwrap();

        assert_eq!(notes_width - todos_width, COL_STATUS as u64);
    }

    #[test]
    fn test_line_width_at_least_min() {
        // line_width() should always be >= MIN_LINE_WIDTH
        let w = line_width();
        assert!(
            w >= MIN_LINE_WIDTH,
            "line_width() = {w}, expected >= {MIN_LINE_WIDTH}"
        );
    }

    #[test]
    fn test_title_width_plus_fixed_equals_line_width() {
        // Verify the column-sum invariant: fixed cols + title_width == line_width()
        let pad = make_pad("Test", false);

        // Notes mode (no status column)
        let dp = make_display_pad(pad.clone(), DisplayIndex::Regular(1));
        let data = build_list_view(&list_result(vec![dp], notes_request()));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        let title_width = pads[0].get("title_width").and_then(|v| v.as_u64()).unwrap() as usize;
        let col_status = row_col_status(&pads[0]) as usize;

        let total = COL_LEFT_PIN + col_status + COL_INDEX + title_width + COL_TIME;
        let w = line_width();
        assert_eq!(total, w, "Notes: columns sum {total} != line_width {w}");

        // Todos mode (with status column)
        let dp2 = make_display_pad(pad, DisplayIndex::Regular(1));
        let data2 = build_list_view(&list_result(vec![dp2], todos_request()));

        let pads2 = data2.get("pads").and_then(|v| v.as_array()).unwrap();
        let title_width2 = pads2[0]
            .get("title_width")
            .and_then(|v| v.as_u64())
            .unwrap() as usize;
        let col_status2 = row_col_status(&pads2[0]) as usize;

        let total2 = COL_LEFT_PIN + col_status2 + COL_INDEX + title_width2 + COL_TIME;
        assert_eq!(total2, w, "Todos: columns sum {total2} != line_width {w}");
    }

    fn make_display_pad_with_children(
        pad: Pad,
        index: DisplayIndex,
        children: Vec<DisplayPad>,
    ) -> DisplayPad {
        DisplayPad {
            pad,
            index,
            matches: None,
            children,
        }
    }

    #[test]
    fn test_build_list_nested_pad_produces_indent() {
        let child = make_display_pad(make_pad("Child Note", false), DisplayIndex::Regular(1));
        let parent = make_display_pad_with_children(
            make_pad("Parent Note", false),
            DisplayIndex::Regular(1),
            vec![child],
        );

        let data = build_list_view(&list_result(vec![parent], todos_request()));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        // Should be flattened: parent + child = 2 entries
        assert_eq!(pads.len(), 2, "parent + child should produce 2 pad entries");

        // Parent at depth 0: no indent
        let parent_indent = pads[0].get("indent").and_then(|v| v.as_str()).unwrap();
        assert_eq!(parent_indent, "", "root pad should have empty indent");

        // Child at depth 1: 2-space indent
        let child_indent = pads[1].get("indent").and_then(|v| v.as_str()).unwrap();
        assert_eq!(
            child_indent, "  ",
            "depth-1 child should have 2-space indent"
        );
    }

    /// `_match_lines.jinja` and `_peek_content.jinja` prefix their lines with
    /// `pad.indent`, so every row must carry it — including rows built for a
    /// listing that requested peek previews.
    #[test]
    fn test_build_list_peek_rows_carry_indent_for_partials() {
        let child = make_display_pad(make_pad("Child Note", false), DisplayIndex::Regular(1));
        let parent = make_display_pad_with_children(
            make_pad("Parent Note", false),
            DisplayIndex::Regular(1),
            vec![child],
        );
        let request = ListRequest {
            peek: true,
            ..Default::default()
        };

        let data = build_list_view(&list_result(vec![parent], request));
        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();

        for (row, expected) in pads.iter().zip(["", "  "]) {
            let indent = row.get("indent").and_then(|v| v.as_str());
            assert_eq!(
                indent,
                Some(expected),
                "peek row must carry the indent its partials prefix with"
            );
        }
    }

    /// `_peek_content.jinja` feeds `indent_width` to the `indent()` filter so a
    /// nested pad's continuation lines stay flush with its own first line. It must
    /// therefore be present and agree with the `indent` string.
    #[test]
    fn test_build_list_row_indent_width_matches_indent_string() {
        let grandchild = make_display_pad(make_pad("Grandchild", false), DisplayIndex::Regular(1));
        let child = make_display_pad_with_children(
            make_pad("Child", false),
            DisplayIndex::Regular(1),
            vec![grandchild],
        );
        let parent = make_display_pad_with_children(
            make_pad("Parent", false),
            DisplayIndex::Regular(1),
            vec![child],
        );

        let data = build_list_view(&list_result(vec![parent], todos_request()));
        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(pads.len(), 3, "parent + child + grandchild");

        for (depth, row) in pads.iter().enumerate() {
            let indent = row.get("indent").and_then(|v| v.as_str()).unwrap();
            let indent_width = row
                .get("indent_width")
                .and_then(|v| v.as_u64())
                .unwrap_or_else(|| panic!("row at depth {depth} must carry indent_width"));

            assert_eq!(
                indent_width as usize,
                indent.len(),
                "indent_width must agree with the indent string at depth {depth}"
            );
            assert_eq!(
                indent_width,
                depth as u64 * 2,
                "each nesting level adds 2 columns"
            );
        }
    }

    /// Modification rows are flat, so their indent must stay zero-width — this pins
    /// the `_pad_line.jinja` prefix for the modification path.
    #[test]
    fn test_build_modification_rows_have_zero_indent() {
        let dp = make_display_pad(make_pad("Note", false), DisplayIndex::Regular(1));
        let data = build_modification_view(&modification_result("Created", vec![dp], false));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(pads[0].get("indent").and_then(|v| v.as_str()), Some(""));
        assert_eq!(
            pads[0].get("indent_width").and_then(|v| v.as_u64()),
            Some(0),
            "modification rows are flat"
        );
    }

    #[test]
    fn test_build_list_nested_title_width_reduced_by_indent() {
        let child = make_display_pad(make_pad("Child", false), DisplayIndex::Regular(1));
        let parent = make_display_pad_with_children(
            make_pad("Parent", false),
            DisplayIndex::Regular(1),
            vec![child],
        );

        let data = build_list_view(&list_result(vec![parent], todos_request()));
        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();

        let parent_width = pads[0].get("title_width").and_then(|v| v.as_u64()).unwrap();
        let child_width = pads[1].get("title_width").and_then(|v| v.as_u64()).unwrap();

        // Child title_width should be exactly 2 less than parent (indent = depth * 2)
        assert_eq!(
            parent_width - child_width,
            2,
            "child title_width should be 2 less than parent"
        );
    }

    #[test]
    fn test_build_list_deep_nesting_indent_accumulates() {
        let grandchild = make_display_pad(make_pad("Grandchild", false), DisplayIndex::Regular(1));
        let child = make_display_pad_with_children(
            make_pad("Child", false),
            DisplayIndex::Regular(1),
            vec![grandchild],
        );
        let parent = make_display_pad_with_children(
            make_pad("Parent", false),
            DisplayIndex::Regular(1),
            vec![child],
        );

        let data = build_list_view(&list_result(vec![parent], todos_request()));
        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();

        assert_eq!(pads.len(), 3, "3-level tree should produce 3 entries");

        let indents: Vec<&str> = pads
            .iter()
            .map(|p| p.get("indent").and_then(|v| v.as_str()).unwrap())
            .collect();
        assert_eq!(indents, vec!["", "  ", "    "]);
    }

    #[test]
    fn test_build_list_nested_preserves_order_parent_then_children() {
        let child_a = make_display_pad(make_pad("Alpha", false), DisplayIndex::Regular(2));
        let child_b = make_display_pad(make_pad("Beta", false), DisplayIndex::Regular(1));
        let parent = make_display_pad_with_children(
            make_pad("Root", false),
            DisplayIndex::Regular(1),
            vec![child_b, child_a],
        );

        let data = build_list_view(&list_result(vec![parent], todos_request()));
        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();

        let titles: Vec<&str> = pads
            .iter()
            .map(|p| p.get("title").and_then(|v| v.as_str()).unwrap())
            .collect();
        assert_eq!(titles, vec!["Root", "Beta", "Alpha"]);
    }

    #[test]
    fn test_build_list_nested_pin_marker_only_at_root() {
        let child = make_display_pad(make_pad("Child", true), DisplayIndex::Pinned(1));
        let parent = make_display_pad_with_children(
            make_pad("Parent", true),
            DisplayIndex::Pinned(1),
            vec![child],
        );

        let data = build_list_view(&list_result(vec![parent], todos_request()));
        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();

        let parent_pin = pads[0].get("left_pin").and_then(|v| v.as_str()).unwrap();
        let child_pin = pads[1].get("left_pin").and_then(|v| v.as_str()).unwrap();

        assert_eq!(parent_pin, PIN_MARKER, "root pinned pad should show marker");
        assert_eq!(child_pin, "", "nested pinned pad should NOT show marker");
    }

    #[test]
    fn test_modification_result_title_width_invariant() {
        let dp = make_display_pad(make_pad("Test", false), DisplayIndex::Regular(1));
        let data = build_modification_view(&modification_result("Created", vec![dp], false));

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        let title_width = pads[0].get("title_width").and_then(|v| v.as_u64()).unwrap() as usize;

        let total = COL_LEFT_PIN + COL_INDEX + title_width + COL_TIME;
        let w = line_width();
        assert_eq!(
            total, w,
            "Modification result: columns sum {total} != line_width {w}"
        );
    }

    // --- Provider shape-matching -------------------------------------------------
    //
    // Each provider must claim only its own command's result. A provider that
    // matched the wrong shape would inject a bogus view into an unrelated template.

    #[test]
    fn test_list_provider_rejects_modification_result() {
        let dp = make_display_pad(make_pad("Test", false), DisplayIndex::Regular(1));
        let data = serde_json::to_value(modification_result("Created", vec![dp], false)).unwrap();

        assert!(
            serde_json::from_value::<PadListResult>(data).is_err(),
            "a modification result must not deserialize as a list result"
        );
    }

    #[test]
    fn test_modification_provider_rejects_list_result() {
        let dp = make_display_pad(make_pad("Test", false), DisplayIndex::Regular(1));
        let data = serde_json::to_value(list_result(vec![dp], todos_request())).unwrap();

        assert!(
            serde_json::from_value::<ModificationResult>(data).is_err(),
            "a list result must not deserialize as a modification result"
        );
    }

    #[test]
    fn test_results_round_trip_through_serialization() {
        // Providers only ever see the serialized handler value, so every result type
        // must survive the round trip its provider performs.
        let dp = make_display_pad(make_pad("Test", false), DisplayIndex::Regular(1));
        let original = list_result(vec![dp], todos_request());

        let data = serde_json::to_value(&original).unwrap();
        let restored: PadListResult = serde_json::from_value(data).unwrap();

        assert_eq!(restored.pads.len(), 1);
        assert_eq!(restored.pads[0].pad.metadata.title, "Test");
        assert_eq!(restored.request, todos_request());
    }
}
