//! # Rendering Module
//!
//! This module builds data structures for standout's template-based rendering.
//! The CLI layer uses standout's `App` for actual rendering via templates and styles.
//!
//! ## Architecture
//!
//! - Handlers call `build_*_value` functions to create `serde_json::Value` data
//! - Handlers return `Output::Render(value)` to standout's dispatch pipeline
//! - Standout renders using templates embedded via `embed_templates!` macro
//! - Structured output modes (JSON, YAML) are handled automatically
//!
//! ## Table Layout
//!
//! The list view uses standout's `col()` filter for declarative column layout. Each row has:
//! - `left_pin` (2 chars): Pin marker for pinned pads (both sections) or empty
//! - `status_icon` (2 chars): Todo status indicator
//! - `index` (4 chars): Display index (p1., 1., d1.)
//! - `title` (fill): Pad title, truncated to fit
//! - `time_ago` (14 chars, right-aligned): Relative timestamp
//!
//! Column widths are defined as constants and the title width is calculated per-row based on
//! the variable prefix width (which depends on section type and nesting depth).
//!

use super::setup::get_grouped_help;
use chrono::{DateTime, Utc};
use padzapp::api::{CmdMessage, MessageLevel, TodoStatus};
use padzapp::index::{DisplayIndex, DisplayPad};
use padzapp::peek::format_as_peek;
use standout::{truncate_to_width, OutputMode};

/// Configuration for list rendering.
pub const LINE_WIDTH: usize = 100;
pub const PIN_MARKER: &str = "⚲";

/// Column widths for list layout (used by standout's `col()` filter)
pub const COL_LEFT_PIN: usize = 2; // Pin marker or empty ("⚲ " or "  ")
pub const COL_STATUS: usize = 2; // Status icon + space
pub const COL_INDEX: usize = 4; // "p1.", " 1.", "d1."
pub const COL_TIME: usize = 5; // Compact timestamp ("34s ⏲")

/// Status indicators for todo status
pub const STATUS_PLANNED: &str = "⚪︎";
pub const STATUS_IN_PROGRESS: &str = "☉︎︎";
pub const STATUS_DONE: &str = "⚫︎";

/// Builds modification result data as serde_json::Value for Dispatch handlers.
///
/// This function creates the appropriate data structure based on output mode:
/// - For structured modes (JSON, YAML): Clean API-friendly format with just action, pads, messages
/// - For terminal modes: Full template-ready format with column widths and transformed pad data
///
/// Used by LocalApp handlers that need to return data for standout's rendering pipeline.
pub fn build_modification_result_value(
    action_verb: &str,
    pads: &[DisplayPad],
    trailing_messages: &[CmdMessage],
    output_mode: OutputMode,
) -> serde_json::Value {
    use serde_json::json;

    // For structured modes, return clean API format
    if output_mode.is_structured() {
        return json!({
            "action": action_verb,
            "pads": pads,
            "messages": trailing_messages,
        });
    }

    // For terminal modes, build full template data
    let count = pads.len();
    let start_message = if count == 0 {
        String::new()
    } else {
        let pad_word = if count == 1 { "pad" } else { "pads" };
        format!("{} {} {}...", action_verb, count, pad_word)
    };

    // Convert pads to template-ready format
    let pad_lines: Vec<serde_json::Value> = pads
        .iter()
        .map(|dp| {
            let is_pinned_section = matches!(dp.index, DisplayIndex::Pinned(_));
            let is_deleted = matches!(dp.index, DisplayIndex::Deleted(_));

            let local_idx_str = match &dp.index {
                DisplayIndex::Pinned(n) => format!("p{}", n),
                DisplayIndex::Regular(n) => format!("{:2}", n),
                DisplayIndex::Deleted(n) => format!("d{}", n),
            };
            let full_idx_str = format!("{}.", local_idx_str);

            let status_icon = match dp.pad.metadata.status {
                TodoStatus::Planned => STATUS_PLANNED,
                TodoStatus::InProgress => STATUS_IN_PROGRESS,
                TodoStatus::Done => STATUS_DONE,
            };

            let left_pin = if dp.pad.metadata.is_pinned {
                PIN_MARKER.to_string()
            } else {
                String::new()
            };

            // Calculate tags display width
            let tags_width = if dp.pad.metadata.tags.is_empty() {
                0
            } else {
                use unicode_width::UnicodeWidthStr;
                let tag_chars: usize = dp.pad.metadata.tags.iter().map(|t| t.width()).sum();
                let spaces = dp.pad.metadata.tags.len().saturating_sub(1);
                tag_chars + spaces + 1
            };

            let fixed_columns = COL_LEFT_PIN + COL_STATUS + COL_INDEX + COL_TIME;
            let title_width = LINE_WIDTH.saturating_sub(fixed_columns + tags_width);

            json!({
                "indent": "",
                "left_pin": left_pin,
                "status_icon": status_icon,
                "index": full_idx_str,
                "title": dp.pad.metadata.title,
                "title_width": title_width,
                "tags": dp.pad.metadata.tags,
                "time_ago": format_time_ago(dp.pad.metadata.created_at),
                "is_pinned_section": is_pinned_section,
                "is_deleted": is_deleted,
                "is_separator": false,
                "matches": [],
                "more_matches_count": 0,
                "peek": serde_json::Value::Null,
            })
        })
        .collect();

    // Convert trailing messages
    let trailing_data = convert_messages_to_json(trailing_messages);

    json!({
        "start_message": start_message,
        "pads": pad_lines,
        "trailing_messages": trailing_data,
        "peek": false,
        "pin_marker": PIN_MARKER,
        "col_left_pin": COL_LEFT_PIN,
        "col_status": COL_STATUS,
        "col_index": COL_INDEX,
        "col_time": COL_TIME,
    })
}

/// Builds list result data as serde_json::Value for Dispatch handlers.
///
/// This function creates the appropriate data structure based on output mode:
/// - For structured modes (JSON, YAML): Clean API-friendly format with just pads
/// - For terminal modes: Full template-ready format with column widths and transformed pad data
///
/// Used by list/search handlers that need to return data for standout's rendering pipeline.
pub fn build_list_result_value(
    pads: &[DisplayPad],
    peek: bool,
    show_deleted_help: bool,
    trailing_messages: &[CmdMessage],
    output_mode: OutputMode,
) -> serde_json::Value {
    use serde_json::json;

    // For structured modes, return clean API format
    if output_mode.is_structured() {
        return json!({
            "pads": pads,
            "messages": trailing_messages,
        });
    }

    // For terminal modes, build full template data
    let trailing_data = convert_messages_to_json(trailing_messages);

    if pads.is_empty() {
        return json!({
            "pads": [],
            "empty": true,
            "pin_marker": PIN_MARKER,
            "help_text": get_grouped_help(),
            "deleted_help": false,
            "peek": false,
            "col_left_pin": COL_LEFT_PIN,
            "col_status": COL_STATUS,
            "col_index": COL_INDEX,
            "col_time": COL_TIME,
            "trailing_messages": trailing_data,
        });
    }

    let mut pad_lines: Vec<serde_json::Value> = Vec::new();
    let mut last_was_pinned = false;

    // Recursive helper to flatten the tree with depth/indentation
    fn process_pad_to_json(
        dp: &DisplayPad,
        pad_lines: &mut Vec<serde_json::Value>,
        depth: usize,
        is_pinned_section: bool,
        is_deleted_root: bool,
        peek: bool,
    ) {
        let is_deleted = matches!(dp.index, DisplayIndex::Deleted(_));

        let local_idx_str = match &dp.index {
            DisplayIndex::Pinned(n) => format!("p{}", n),
            DisplayIndex::Regular(n) => format!("{:2}", n),
            DisplayIndex::Deleted(n) => format!("d{}", n),
        };
        let full_idx_str = format!("{}.", local_idx_str);

        let status_icon = match dp.pad.metadata.status {
            TodoStatus::Planned => STATUS_PLANNED,
            TodoStatus::InProgress => STATUS_IN_PROGRESS,
            TodoStatus::Done => STATUS_DONE,
        };

        let indent_width = depth * 2;
        let indent = " ".repeat(indent_width);

        let left_pin = if dp.pad.metadata.is_pinned && depth == 0 {
            PIN_MARKER.to_string()
        } else {
            String::new()
        };

        // Calculate tags display width
        let tags_width = if dp.pad.metadata.tags.is_empty() {
            0
        } else {
            use unicode_width::UnicodeWidthStr;
            let tag_chars: usize = dp.pad.metadata.tags.iter().map(|t| t.width()).sum();
            let spaces = dp.pad.metadata.tags.len().saturating_sub(1);
            tag_chars + spaces + 1
        };

        let fixed_columns = COL_LEFT_PIN + COL_STATUS + COL_INDEX + COL_TIME;
        let title_width = LINE_WIDTH.saturating_sub(fixed_columns + indent_width + tags_width);

        // Process matches
        let mut match_lines: Vec<serde_json::Value> = Vec::new();
        if let Some(matches) = &dp.matches {
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

                let match_indent = indent_width + COL_LEFT_PIN + COL_STATUS + COL_INDEX;
                let match_available = LINE_WIDTH.saturating_sub(COL_TIME + match_indent);

                // Truncate segments to available width
                let truncated = truncate_match_segments_to_json(&segments, match_available);

                match_lines.push(serde_json::json!({
                    "line_number": format!("{:02}", m.line_number),
                    "segments": truncated,
                }));
            }
        }

        let peek_data = if peek {
            let body_lines: Vec<&str> = dp.pad.content.lines().skip(1).collect();
            let body = body_lines.join("\n");
            let result = format_as_peek(&body, 3);
            if result.opening_lines.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::to_value(&result).unwrap_or(serde_json::Value::Null)
            }
        } else {
            serde_json::Value::Null
        };

        pad_lines.push(serde_json::json!({
            "indent": indent,
            "left_pin": left_pin,
            "status_icon": status_icon,
            "index": full_idx_str,
            "title": dp.pad.metadata.title,
            "title_width": title_width,
            "tags": dp.pad.metadata.tags,
            "time_ago": format_time_ago(dp.pad.metadata.created_at),
            "is_pinned_section": is_pinned_section && depth == 0,
            "is_deleted": is_deleted || is_deleted_root,
            "is_separator": false,
            "matches": match_lines,
            "more_matches_count": 0,
            "peek": peek_data,
        }));

        // Recurse children
        for child in &dp.children {
            process_pad_to_json(
                child,
                pad_lines,
                depth + 1,
                is_pinned_section,
                is_deleted_root,
                peek,
            );
        }
    }

    for dp in pads {
        let is_pinned_section = matches!(dp.index, DisplayIndex::Pinned(_));
        let is_deleted_section = matches!(dp.index, DisplayIndex::Deleted(_));

        // Separator between pinned and regular roots
        if last_was_pinned && !is_pinned_section {
            pad_lines.push(serde_json::json!({
                "indent": "",
                "left_pin": "",
                "status_icon": "",
                "index": "",
                "title": "",
                "title_width": 0,
                "tags": [],
                "time_ago": "",
                "is_pinned_section": false,
                "is_deleted": false,
                "is_separator": true,
                "matches": [],
                "more_matches_count": 0,
                "peek": serde_json::Value::Null,
            }));
        }
        last_was_pinned = is_pinned_section;

        process_pad_to_json(
            dp,
            &mut pad_lines,
            0,
            is_pinned_section,
            is_deleted_section,
            peek,
        );
    }

    json!({
        "pads": pad_lines,
        "empty": false,
        "pin_marker": PIN_MARKER,
        "help_text": "",
        "deleted_help": show_deleted_help,
        "peek": peek,
        "col_left_pin": COL_LEFT_PIN,
        "col_status": COL_STATUS,
        "col_index": COL_INDEX,
        "col_time": COL_TIME,
        "trailing_messages": trailing_data,
    })
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
    use padzapp::model::Pad;
    use standout::OutputMode;

    fn make_pad(title: &str, pinned: bool, deleted: bool) -> Pad {
        let mut p = Pad::new(title.to_string(), "some content".to_string());
        p.metadata.is_pinned = pinned;
        p.metadata.is_deleted = deleted;
        p
    }

    fn make_display_pad(pad: Pad, index: DisplayIndex) -> DisplayPad {
        DisplayPad {
            pad,
            index,
            matches: None,
            children: vec![],
        }
    }

    #[test]
    fn test_build_list_empty() {
        let data = build_list_result_value(&[], false, false, &[], OutputMode::Text);
        assert!(data.get("empty").and_then(|v| v.as_bool()).unwrap_or(false));
        assert!(data
            .get("help_text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .contains("create"));
    }

    #[test]
    fn test_build_list_single_regular_pad() {
        let pad = make_pad("Test Note", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        let data = build_list_result_value(&[dp], false, false, &[], OutputMode::Text);

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
        let pad = make_pad("Pinned Note", true, false);
        let dp = make_display_pad(pad, DisplayIndex::Pinned(1));

        let data = build_list_result_value(&[dp], false, false, &[], OutputMode::Text);

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
        let pad = make_pad("Deleted Note", false, true);
        let dp = make_display_pad(pad, DisplayIndex::Deleted(1));

        let data = build_list_result_value(&[dp], false, true, &[], OutputMode::Text);

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
        let pinned = make_pad("Pinned", true, false);
        let regular = make_pad("Regular", false, false);

        let pads = vec![
            make_display_pad(pinned.clone(), DisplayIndex::Pinned(1)),
            make_display_pad(regular, DisplayIndex::Regular(1)),
        ];

        let data = build_list_result_value(&pads, false, false, &[], OutputMode::Text);

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
        let mut pad = make_pad("Pinned Note", true, false);
        pad.metadata.is_pinned = true;

        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        let data = build_list_result_value(&[dp], false, false, &[], OutputMode::Text);

        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        let pad_data = &pads[0];

        assert_eq!(
            pad_data.get("left_pin").and_then(|v| v.as_str()),
            Some(PIN_MARKER)
        );
    }

    #[test]
    fn test_build_list_with_messages() {
        let pad = make_pad("Test", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));
        let messages = vec![CmdMessage::success("Operation completed")];

        let data = build_list_result_value(&[dp], false, false, &messages, OutputMode::Text);

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
    fn test_build_list_json_mode_returns_raw_pads() {
        let pad = make_pad("Test", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        let data = build_list_result_value(&[dp], false, false, &[], OutputMode::Json);

        // In JSON mode, should return raw pads array, not processed pad lines
        let pads = data.get("pads").and_then(|v| v.as_array()).unwrap();
        assert_eq!(pads.len(), 1);
        // Raw pads have "pad" field with content
        assert!(pads[0].get("pad").is_some());
    }

    #[test]
    fn test_build_modification_result() {
        let pad = make_pad("Test", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        let data = build_modification_result_value("Created", &[dp], &[], OutputMode::Text);

        assert_eq!(
            data.get("start_message").and_then(|v| v.as_str()),
            Some("Created 1 pad...")
        );
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
}
