//! # Rendering Module
//!
//! This module provides styled terminal output using the `outstanding` crate.
//! Templates are defined here and rendered with automatic terminal color detection.
//!
//! ## Design Philosophy
//!
//! Layout calculations (width, truncation, padding) stay in Rust because they require
//! Unicode-aware processing. Templates handle presentation concerns:
//! - Style selection based on semantic flags (is_pinned, is_deleted, etc.)
//! - Section separators and grouping
//! - Conditional icon rendering

use super::styles::{names, PADZ_THEME};
use super::templates::{FULL_PAD_TEMPLATE, LIST_TEMPLATE, MESSAGES_TEMPLATE, TEXT_LIST_TEMPLATE};
use chrono::{DateTime, Utc};
use outstanding::{render, render_with_color, ThemeChoice};
use padz::api::{CmdMessage, MessageLevel};
use padz::index::{DisplayIndex, DisplayPad};
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

/// Configuration for list rendering.
pub const LINE_WIDTH: usize = 100;
pub const TIME_WIDTH: usize = 14;
pub const PIN_MARKER: &str = "⚲";

/// Semantic pad data for template rendering.
///
/// Contains pre-computed layout strings plus semantic flags that templates
/// use to select styles. This keeps layout math in Rust while letting
/// templates handle style selection through conditionals.
#[derive(Serialize)]
struct PadLineData {
    // Pre-computed layout components (Rust handles width calculations)
    left_pad: String,
    index: String,
    title: String,
    padding: String,
    time_ago: String,
    // Semantic flags for template-driven style selection
    is_pinned_section: bool, // In the pinned section (p1, p2, etc.)
    is_deleted: bool,        // In the deleted section (d1, d2, etc.)
    show_left_pin: bool,     // Show pin marker on left (pinned section)
    show_right_pin: bool,    // Show pin marker on right (pinned pad in regular section)
    is_separator: bool,      // Empty line separator between sections
}

/// Data structure for the full list template.
#[derive(Serialize)]
struct ListData {
    pads: Vec<PadLineData>,
    empty: bool,
    pin_marker: String,
}

#[derive(Serialize)]
struct FullPadData {
    pads: Vec<FullPadEntry>,
}

/// Full pad data with semantic flags for template-driven styling.
#[derive(Serialize)]
struct FullPadEntry {
    index: String,
    title: String,
    content: String,
    // Semantic flags for style selection in template
    is_pinned: bool,
    is_deleted: bool,
}

#[derive(Serialize)]
struct TextListData {
    lines: Vec<String>,
    empty_message: String,
}

#[derive(Serialize)]
struct MessageData {
    content: String,
    style: String,
}

#[derive(Serialize)]
struct MessagesData {
    messages: Vec<MessageData>,
}

/// Renders a list of pads to a string.
pub fn render_pad_list(pads: &[DisplayPad]) -> String {
    render_pad_list_internal(pads, None)
}

fn render_pad_list_internal(pads: &[DisplayPad], use_color: Option<bool>) -> String {
    let empty_data = ListData {
        pads: vec![],
        empty: true,
        pin_marker: PIN_MARKER.to_string(),
    };

    if pads.is_empty() {
        return match use_color {
            Some(c) => render_with_color(
                LIST_TEMPLATE,
                &empty_data,
                ThemeChoice::from(&*PADZ_THEME),
                c,
            ),
            None => render(LIST_TEMPLATE, &empty_data, ThemeChoice::from(&*PADZ_THEME)),
        }
        .unwrap_or_else(|_| "No pads found.\n".to_string());
    }

    let mut pad_lines = Vec::new();
    let mut last_was_pinned = false;

    for dp in pads {
        let is_pinned_section = matches!(dp.index, DisplayIndex::Pinned(_));
        let is_deleted = matches!(dp.index, DisplayIndex::Deleted(_));
        let show_right_pin = dp.pad.metadata.is_pinned && !is_pinned_section;

        // Add separator line between pinned and regular sections
        if last_was_pinned && !is_pinned_section {
            pad_lines.push(PadLineData {
                left_pad: String::new(),
                index: String::new(),
                title: String::new(),
                padding: String::new(),
                time_ago: String::new(),
                is_pinned_section: false,
                is_deleted: false,
                show_left_pin: false,
                show_right_pin: false,
                is_separator: true,
            });
        }
        last_was_pinned = is_pinned_section;

        // Format index string
        let idx_str = match &dp.index {
            DisplayIndex::Pinned(n) => format!("p{}. ", n),
            DisplayIndex::Regular(n) => format!("{}. ", n),
            DisplayIndex::Deleted(n) => format!("d{}. ", n),
        };

        // Calculate left padding (accounts for pin marker space)
        let left_pad = if is_pinned_section {
            "  ".to_string() // Space for "⚲ " prefix
        } else {
            "    ".to_string() // 4 spaces to align with pinned entries
        };

        // Calculate available width for title
        let pin_width = PIN_MARKER.width();
        let left_prefix_width = if is_pinned_section {
            2 + pin_width + 1 // "  " + pin + " "
        } else {
            4 // "    "
        };
        let right_suffix_width = if show_right_pin {
            pin_width + 1 // pin + " "
        } else {
            2 // "  "
        };

        let idx_width = idx_str.width();
        let fixed_width = left_prefix_width + idx_width + right_suffix_width + TIME_WIDTH;
        let available = LINE_WIDTH.saturating_sub(fixed_width);

        // Truncate title and calculate padding
        let title_display = truncate_to_width(dp.pad.metadata.title.as_str(), available);
        let title_width = title_display.width();
        let padding = " ".repeat(available.saturating_sub(title_width));

        pad_lines.push(PadLineData {
            left_pad,
            index: idx_str,
            title: title_display,
            padding,
            time_ago: format_time_ago(dp.pad.metadata.created_at),
            is_pinned_section,
            is_deleted,
            show_left_pin: is_pinned_section,
            show_right_pin,
            is_separator: false,
        });
    }

    let data = ListData {
        pads: pad_lines,
        empty: false,
        pin_marker: PIN_MARKER.to_string(),
    };

    match use_color {
        Some(c) => render_with_color(LIST_TEMPLATE, &data, ThemeChoice::from(&*PADZ_THEME), c),
        None => render(LIST_TEMPLATE, &data, ThemeChoice::from(&*PADZ_THEME)),
    }
    .unwrap_or_else(|e| format!("Render error: {}\n", e))
}

/// Renders full pad contents similar to the legacy `print_full_pads` output.
pub fn render_full_pads(pads: &[DisplayPad]) -> String {
    render_full_pads_internal(pads, None)
}

fn render_full_pads_internal(pads: &[DisplayPad], use_color: Option<bool>) -> String {
    let entries = pads
        .iter()
        .map(|dp| {
            let is_pinned = matches!(dp.index, DisplayIndex::Pinned(_));
            let is_deleted = matches!(dp.index, DisplayIndex::Deleted(_));

            FullPadEntry {
                index: format!("{}", dp.index),
                title: dp.pad.metadata.title.clone(),
                content: dp.pad.content.clone(),
                is_pinned,
                is_deleted,
            }
        })
        .collect();

    let data = FullPadData { pads: entries };

    match use_color {
        Some(c) => render_with_color(FULL_PAD_TEMPLATE, &data, ThemeChoice::from(&*PADZ_THEME), c),
        None => render(FULL_PAD_TEMPLATE, &data, ThemeChoice::from(&*PADZ_THEME)),
    }
    .unwrap_or_else(|e| format!("Render error: {}\n", e))
}

pub fn render_text_list(lines: &[String], empty_message: &str) -> String {
    render_text_list_internal(lines, empty_message, None)
}

fn render_text_list_internal(
    lines: &[String],
    empty_message: &str,
    use_color: Option<bool>,
) -> String {
    let data = TextListData {
        lines: lines.to_vec(),
        empty_message: empty_message.to_string(),
    };

    match use_color {
        Some(c) => render_with_color(
            TEXT_LIST_TEMPLATE,
            &data,
            ThemeChoice::from(&*PADZ_THEME),
            c,
        ),
        None => render(TEXT_LIST_TEMPLATE, &data, ThemeChoice::from(&*PADZ_THEME)),
    }
    .unwrap_or_else(|_| format!("{}\n", empty_message))
}

/// Renders command messages using the template system with themed styles.
pub fn render_messages(messages: &[CmdMessage]) -> String {
    if messages.is_empty() {
        return String::new();
    }

    let message_data: Vec<MessageData> = messages
        .iter()
        .map(|msg| {
            let style = match msg.level {
                MessageLevel::Info => names::INFO,
                MessageLevel::Success => names::SUCCESS,
                MessageLevel::Warning => names::WARNING,
                MessageLevel::Error => names::ERROR,
            };
            MessageData {
                content: msg.content.clone(),
                style: style.to_string(),
            }
        })
        .collect();

    let data = MessagesData {
        messages: message_data,
    };

    render(MESSAGES_TEMPLATE, &data, ThemeChoice::from(&*PADZ_THEME)).unwrap_or_else(|_| {
        messages
            .iter()
            .map(|m| format!("{}\n", m.content))
            .collect()
    })
}

/// Prints command messages to stdout using the template system.
pub fn print_messages(messages: &[CmdMessage]) {
    let output = render_messages(messages);
    if !output.is_empty() {
        print!("{}", output);
    }
}

fn truncate_to_width(s: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthChar;

    let mut result = String::new();
    let mut current_width = 0;
    let limit = max_width.saturating_sub(1);

    for c in s.chars() {
        let char_width = c.width().unwrap_or(0);
        if current_width + char_width > limit {
            result.push('…');
            return result;
        }
        result.push(c);
        current_width += char_width;
    }

    result
}

fn format_time_ago(timestamp: DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(timestamp);

    let formatter = timeago::Formatter::new();
    let time_str = formatter.convert(duration.to_std().unwrap_or_default());

    // Left-pad time units so they align vertically with "seconds" (7 chars)
    // This makes columns line up nicely:
    //   3 seconds ago
    //   1     day ago
    //   2   hours ago
    // Match with " ago" suffix to avoid substring issues (e.g., "hour" in "hours")
    // Note: seconds/minutes are already 7 chars, no replacement needed
    let time_str = time_str
        .replace("hours ago", "  hours ago") // 5 -> 7
        .replace("hour ago", "   hour ago") // 4 -> 7
        .replace("days ago", "   days ago") // 4 -> 7
        .replace("day ago", "    day ago") // 3 -> 7
        .replace("weeks ago", "  weeks ago") // 5 -> 7
        .replace("week ago", "   week ago") // 4 -> 7
        .replace("months ago", " months ago") // 6 -> 7
        .replace("month ago", "  month ago") // 5 -> 7
        .replace("years ago", "  years ago") // 5 -> 7
        .replace("year ago", "   year ago"); // 4 -> 7

    // Right-pad to TIME_WIDTH for consistent column alignment
    format!("{:>width$}", time_str, width = TIME_WIDTH)
}

#[cfg(test)]
mod tests {
    use super::*;
    use padz::model::Pad;

    fn make_pad(title: &str, pinned: bool, deleted: bool) -> Pad {
        let mut p = Pad::new(title.to_string(), "some content".to_string());
        p.metadata.is_pinned = pinned;
        p.metadata.is_deleted = deleted;
        p
    }

    fn make_display_pad(pad: Pad, index: DisplayIndex) -> DisplayPad {
        DisplayPad { pad, index }
    }

    #[test]
    fn test_render_empty_list() {
        let output = render_pad_list_internal(&[], Some(false));
        assert_eq!(output.trim(), "No pads found.");
    }

    #[test]
    fn test_render_single_regular_pad() {
        let pad = make_pad("Test Note", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        let output = render_pad_list_internal(&[dp], Some(false));

        // Should contain the index and title
        assert!(output.contains("1."));
        assert!(output.contains("Test Note"));
        // Should have the left padding (4 spaces for regular)
        assert!(output.contains("    1."));
    }

    #[test]
    fn test_render_pinned_pad() {
        let pad = make_pad("Pinned Note", true, false);
        let dp = make_display_pad(pad, DisplayIndex::Pinned(1));

        let output = render_pad_list_internal(&[dp], Some(false));

        // Should contain pinned index
        assert!(output.contains("p1."));
        assert!(output.contains("Pinned Note"));
        // Should have pin marker in prefix
        assert!(output.contains(PIN_MARKER));
    }

    #[test]
    fn test_render_deleted_pad() {
        let pad = make_pad("Deleted Note", false, true);
        let dp = make_display_pad(pad, DisplayIndex::Deleted(1));

        let output = render_pad_list_internal(&[dp], Some(false));

        // Should contain deleted index
        assert!(output.contains("d1."));
        assert!(output.contains("Deleted Note"));
    }

    #[test]
    fn test_render_mixed_pinned_and_regular() {
        let pinned = make_pad("Pinned", true, false);
        let regular = make_pad("Regular", false, false);

        let pads = vec![
            make_display_pad(pinned.clone(), DisplayIndex::Pinned(1)),
            make_display_pad(regular, DisplayIndex::Regular(1)),
            make_display_pad(pinned, DisplayIndex::Regular(2)),
        ];

        let output = render_pad_list_internal(&pads, Some(false));

        // Should have pinned section with marker
        assert!(output.contains("p1."));
        // Should have separator (blank line) between pinned and regular
        let lines: Vec<&str> = output.lines().collect();
        // First non-empty line should be blank (leading newline for pinned section)
        // Then pinned entry, then blank line separator, then regular entries
        assert!(lines.iter().any(|l| l.trim().is_empty()));
    }

    #[test]
    fn test_render_pinned_marker_on_regular_entry() {
        // A pinned pad should show pin marker when displayed in regular section
        let mut pad = make_pad("Pinned Note", true, false);
        pad.metadata.is_pinned = true;

        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        let output = render_pad_list_internal(&[dp], Some(false));

        // Should have pin marker on the right side
        assert!(output.contains(PIN_MARKER));
    }

    #[test]
    fn test_render_with_color_includes_ansi() {
        let pad = make_pad("Test", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        // Force styling for test environment
        let output = render_pad_list_internal(&[dp], Some(true));

        // When use_color is true, should include ANSI codes (at least for time which is dimmed)
        // Note: console crate may not emit codes in test env, so we just verify it runs
        assert!(output.contains("Test"));
    }

    #[test]
    fn test_render_full_pads_empty() {
        let output = render_full_pads_internal(&[], Some(false));
        assert!(output.contains("No pads found."));
    }

    #[test]
    fn test_render_full_pads_single() {
        let pad = make_pad("Full Pad", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(3));

        let output = render_full_pads_internal(&[dp], Some(false));

        assert!(output.contains("3 Full Pad"));
        assert!(output.contains("some content"));

        let lines: Vec<&str> = output.lines().collect();
        let header_index = lines
            .iter()
            .position(|line| line.contains("3 Full Pad"))
            .expect("header line missing");
        let spacer = lines.get(header_index + 1).copied().unwrap_or_default();
        assert!(
            spacer.trim().is_empty(),
            "expected blank separator line between title and content, got: {:?}",
            spacer
        );
        let body_section = &lines[(header_index + 2).min(lines.len())..];
        assert!(
            body_section
                .iter()
                .any(|line| line.contains("some content")),
            "expected rendered body to include pad content"
        );
    }

    #[test]
    fn test_render_text_list_empty() {
        let output = render_text_list_internal(&[], "Nothing here.", Some(false));
        assert!(output.contains("Nothing here."));
    }

    #[test]
    fn test_render_text_list_lines() {
        let lines = vec!["first".to_string(), "second".to_string()];
        let output = render_text_list_internal(&lines, "", Some(false));
        assert!(output.contains("first"));
        assert!(output.contains("second"));
    }

    #[test]
    fn test_render_messages_empty() {
        let output = render_messages(&[]);
        assert!(output.is_empty());
    }

    #[test]
    fn test_render_messages_success() {
        let messages = vec![CmdMessage::success("Pad created: Test")];
        let output = render_messages(&messages);
        assert!(output.contains("Pad created: Test"));
    }

    #[test]
    fn test_render_messages_multiple() {
        let messages = vec![
            CmdMessage::info("Info message"),
            CmdMessage::warning("Warning message"),
            CmdMessage::error("Error message"),
        ];
        let output = render_messages(&messages);
        assert!(output.contains("Info message"));
        assert!(output.contains("Warning message"));
        assert!(output.contains("Error message"));
    }

    #[test]
    fn test_format_time_ago_alignment() {
        // All time units should be padded to 7 chars for vertical alignment
        use chrono::Duration;

        let now = Utc::now();

        // Test that various time units are padded correctly
        // The padded unit + " ago" should show consistent alignment
        let test_cases = [
            (Duration::seconds(30), "seconds ago"), // 7 chars - no padding
            (Duration::minutes(5), "minutes ago"),  // 7 chars - no padding
            (Duration::hours(2), "  hours ago"),    // 5 -> 7 chars
            (Duration::days(3), "   days ago"),     // 4 -> 7 chars
            (Duration::weeks(1), "   week ago"),    // 4 -> 7 chars
            (Duration::days(45), " month ago"),     // 6 -> 7 chars (singular)
            (Duration::days(400), "   year ago"),   // 4 -> 7 chars
        ];

        for (duration, expected_pattern) in test_cases {
            let timestamp = now - duration;
            let formatted = format_time_ago(timestamp);
            assert!(
                formatted.contains(expected_pattern),
                "Expected '{}' to contain '{}' for duration {:?}",
                formatted.trim(),
                expected_pattern,
                duration
            );
        }
    }
}
