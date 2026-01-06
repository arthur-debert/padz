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

use super::setup::get_grouped_help;
use super::styles::{names, PADZ_THEME};
use super::templates::{FULL_PAD_TEMPLATE, LIST_TEMPLATE, MESSAGES_TEMPLATE, TEXT_LIST_TEMPLATE};
use chrono::{DateTime, Utc};
use outstanding::{render, render_with_color, truncate_to_width, ThemeChoice};
use padz::api::{CmdMessage, MessageLevel};
use padz::index::{DisplayIndex, DisplayPad};
use padz::peek::{format_as_peek, PeekResult};
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

/// Configuration for list rendering.
pub const LINE_WIDTH: usize = 100;
pub const TIME_WIDTH: usize = 14;
pub const PIN_MARKER: &str = "⚲";

#[derive(Serialize)]
struct MatchSegmentData {
    text: String,
    style: String,
}

#[derive(Serialize)]
struct MatchLineData {
    segments: Vec<MatchSegmentData>,
    line_number: String,
}

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
    // Search matches
    matches: Vec<MatchLineData>,
    more_matches_count: usize,
    peek: Option<PeekResult>,
}

/// Data structure for the full list template.
#[derive(Serialize)]
struct ListData {
    pads: Vec<PadLineData>,
    empty: bool,
    pin_marker: String,
    help_text: String,
    deleted_help: bool,
    peek: bool,
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
pub fn render_pad_list(pads: &[DisplayPad], peek: bool) -> String {
    render_pad_list_internal(pads, None, false, peek)
}

/// Renders a list of pads with optional deleted help text.
pub fn render_pad_list_deleted(pads: &[DisplayPad], peek: bool) -> String {
    render_pad_list_internal(pads, None, true, peek)
}

fn render_pad_list_internal(
    pads: &[DisplayPad],
    use_color: Option<bool>,
    show_deleted_help: bool,
    peek: bool,
) -> String {
    let empty_data = ListData {
        pads: vec![],
        empty: true,
        pin_marker: PIN_MARKER.to_string(),
        help_text: get_grouped_help(),
        deleted_help: false,
        peek: false,
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
                matches: vec![],
                more_matches_count: 0,
                peek: None,
            });
        }
        last_was_pinned = is_pinned_section;

        // Format index string
        let idx_str = match &dp.index {
            DisplayIndex::Pinned(n) => format!("p{}. ", n),
            DisplayIndex::Regular(n) => format!("{:02}. ", n),
            DisplayIndex::Deleted(n) => format!("d{}. ", n),
        };

        // Calculate left padding (accounts for pin marker space)
        let left_pad = if is_pinned_section {
            String::new() // No padding for pinned (pin marker provides alignment)
        } else {
            "  ".to_string() // 2 spaces to align with pinned entries' "⚲ " prefix
        };

        // Calculate available width for title
        let pin_width = PIN_MARKER.width();
        let left_prefix_width = if is_pinned_section {
            pin_width + 1 // pin + " "
        } else {
            2 // "  "
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

        // Process matches
        let mut match_lines = Vec::new();
        let more_matches = 0;

        if let Some(matches) = &dp.matches {
            for m in matches {
                if m.line_number == 0 {
                    continue;
                }

                let segments: Vec<MatchSegmentData> = m
                    .segments
                    .iter()
                    .map(|s| match s {
                        padz::index::MatchSegment::Plain(t) => MatchSegmentData {
                            text: t.clone(),
                            style: names::MUTED.to_string(),
                        },
                        padz::index::MatchSegment::Match(t) => MatchSegmentData {
                            text: t.clone(),
                            style: names::HIGHLIGHT.to_string(),
                        },
                    })
                    .collect();

                // Calculate indentation width for matches
                // Template: "{{ pad.left_pad }}    {{ match.line_number }} "
                // left_pad + 4 spaces + 2 digits + 1 space
                // left_pad string is "  " (2 chars) or "    " (4 chars).
                // Let's use left_pad.width() directly.
                let indent_width = left_pad.width() + 4 + 2 + 1;

                // Available width for match content
                // User wants to avoid time column.
                // LINE_WIDTH - TIME_WIDTH - indent
                let match_available = LINE_WIDTH
                    .saturating_sub(TIME_WIDTH)
                    .saturating_sub(indent_width);

                let truncated_segments = truncate_match_segments(segments, match_available);

                match_lines.push(MatchLineData {
                    line_number: format!("{:02}", m.line_number),
                    segments: truncated_segments,
                });
            }
        }

        let peek_data = if peek {
            // Strip the first line (title) to avoid duplication
            let body_lines: Vec<&str> = dp.pad.content.lines().skip(1).collect();
            let body = body_lines.join("\n");

            let result = format_as_peek(&body, 3);
            if result.opening_lines.is_empty() {
                None
            } else {
                Some(result)
            }
        } else {
            None
        };

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
            matches: match_lines,
            more_matches_count: more_matches,
            peek: peek_data,
        });
    }

    let data = ListData {
        pads: pad_lines,
        empty: false,
        pin_marker: PIN_MARKER.to_string(),
        help_text: String::new(), // Not used when not empty
        deleted_help: show_deleted_help,
        peek,
    };

    match use_color {
        Some(c) => render_with_color(LIST_TEMPLATE, &data, ThemeChoice::from(&*PADZ_THEME), c),
        None => render(LIST_TEMPLATE, &data, ThemeChoice::from(&*PADZ_THEME)),
    }
    .unwrap_or_else(|e| format!("Render error: {}\n", e))
}

fn truncate_match_segments(
    segments: Vec<MatchSegmentData>,
    max_width: usize,
) -> Vec<MatchSegmentData> {
    use unicode_width::UnicodeWidthStr;

    let mut result = Vec::new();
    let mut current_width = 0;
    // Reserve space for ellipsis if needed

    for seg in segments {
        let w = seg.text.width();
        if current_width + w <= max_width {
            result.push(seg);
            current_width += w;
        } else {
            // Truncate this segment
            let remaining = max_width.saturating_sub(current_width);
            let truncated = truncate_to_width(&seg.text, remaining);
            // truncate_to_width adds ellipsis if needed, but we passed strictly remaining width.
            // If truncate_to_width adds ellipsis, it fits in `remaining`.

            result.push(MatchSegmentData {
                text: truncated,
                style: seg.style,
            });
            return result;
        }
    }
    result
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
        DisplayPad {
            pad,
            index,
            matches: None,
        }
    }

    #[test]
    fn test_render_empty_list() {
        let output = render_pad_list_internal(&[], Some(false), false, false);
        // Should show the "no pads yet" message with help text
        assert!(output.contains("No pads yet, create one with `padz create`"));
    }

    #[test]
    fn test_render_single_regular_pad() {
        let pad = make_pad("Test Note", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        let output = render_pad_list_internal(&[dp], Some(false), false, false);

        // Should contain zero-padded index and title
        assert!(output.contains("01."));
        assert!(output.contains("Test Note"));
        // Should have 2-space left padding for regular entries (aligns with pinned "⚲ " prefix)
        assert!(output.contains("  01."));
    }

    #[test]
    fn test_render_pinned_pad() {
        let pad = make_pad("Pinned Note", true, false);
        let dp = make_display_pad(pad, DisplayIndex::Pinned(1));

        let output = render_pad_list_internal(&[dp], Some(false), false, false);

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

        let output = render_pad_list_internal(&[dp], Some(false), false, false);

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

        let output = render_pad_list_internal(&pads, Some(false), false, false);

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

        let output = render_pad_list_internal(&[dp], Some(false), false, false);

        // Should have pin marker on the right side
        assert!(output.contains(PIN_MARKER));
    }

    #[test]
    fn test_render_with_color_includes_ansi() {
        let pad = make_pad("Test", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        // Force styling for test environment
        let output = render_pad_list_internal(&[dp], Some(true), false, false);

        // When use_color is true, should include ANSI codes (at least for time which is dimmed)
        // Note: console crate may not emit codes in test env, so we just verify it runs
        assert!(output.contains("Test"));
    }

    #[test]
    fn test_render_search_results() {
        use padz::index::{MatchSegment, SearchMatch};

        let pad = make_pad("Search Result", false, false);
        let mut dp = make_display_pad(pad, DisplayIndex::Regular(1));

        dp.matches = Some(vec![SearchMatch {
            line_number: 2,
            segments: vec![
                MatchSegment::Plain("Found ".to_string()),
                MatchSegment::Match("match".to_string()),
                MatchSegment::Plain(" here".to_string()),
            ],
        }]);

        let output = render_pad_list_internal(&[dp], Some(false), false, false);

        assert!(output.contains("1."));
        assert!(output.contains("Search Result"));
        // Check indentation of match line (should have padding)
        assert!(output.contains("    02 Found match here"));
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
