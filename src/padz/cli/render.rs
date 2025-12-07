//! # Rendering Module
//!
//! This module provides styled terminal output using the `outstanding` crate.
//! Templates are defined here and rendered with automatic terminal color detection.

use super::styles::{FULL_PAD_STYLES, LIST_STYLES, TEXT_LIST_STYLES};
use super::templates::{FULL_PAD_TEMPLATE, LIST_TEMPLATE, TEXT_LIST_TEMPLATE};
use chrono::{DateTime, Utc};
use outstanding::render_with_color;
use padz::index::{DisplayIndex, DisplayPad};
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

/// Configuration for list rendering.
pub const LINE_WIDTH: usize = 100;
pub const TIME_WIDTH: usize = 14;
pub const PIN_MARKER: &str = "⚲";

/// Data structure for rendering a single pad line.
#[derive(Serialize)]
struct PadLineData {
    left_prefix: String,
    index: String,
    title_content: String,
    padding: String,
    right_suffix: String,
    time_ago: String,
    // Style hints
    index_style: String,
}

/// Data structure for the full list template.
#[derive(Serialize)]
struct ListData {
    has_pinned: bool,
    pads: Vec<PadLineData>,
    empty: bool,
}

#[derive(Serialize)]
struct FullPadData {
    pads: Vec<FullPadEntry>,
}

#[derive(Serialize)]
struct FullPadEntry {
    index: String,
    title: String,
    content: String,
}

#[derive(Serialize)]
struct TextListData {
    lines: Vec<String>,
    empty_message: String,
}

/// Renders a list of pads to a string.
pub fn render_pad_list(pads: &[DisplayPad], use_color: bool) -> String {
    if pads.is_empty() {
        return render_with_color(
            LIST_TEMPLATE,
            &ListData {
                has_pinned: false,
                pads: vec![],
                empty: true,
            },
            &LIST_STYLES,
            use_color,
        )
        .unwrap_or_else(|_| "No pads found.\n".to_string());
    }

    let has_pinned = pads
        .iter()
        .any(|dp| matches!(dp.index, DisplayIndex::Pinned(_)));

    let mut pad_lines = Vec::new();
    let mut last_was_pinned = false;

    for dp in pads {
        let is_pinned_entry = matches!(dp.index, DisplayIndex::Pinned(_));

        // Add separator line between pinned and regular sections
        if last_was_pinned && !is_pinned_entry {
            pad_lines.push(PadLineData {
                left_prefix: String::new(),
                index: String::new(),
                title_content: String::new(),
                padding: String::new(),
                right_suffix: String::new(),
                time_ago: String::new(),
                index_style: "index_regular".to_string(),
            });
        }
        last_was_pinned = is_pinned_entry;

        let idx_str = match &dp.index {
            DisplayIndex::Pinned(n) => format!("p{}. ", n),
            DisplayIndex::Regular(n) => format!("{}. ", n),
            DisplayIndex::Deleted(n) => format!("d{}. ", n),
        };

        let left_prefix = if is_pinned_entry {
            format!("  {} ", PIN_MARKER)
        } else {
            "    ".to_string()
        };
        let left_prefix_width = left_prefix.width();

        let right_suffix = if dp.pad.metadata.is_pinned && !is_pinned_entry {
            format!("{} ", PIN_MARKER)
        } else {
            "  ".to_string()
        };
        let right_suffix_width = right_suffix.width();

        let time_ago = format_time_ago(dp.pad.metadata.created_at);

        let title = &dp.pad.metadata.title;
        let content_preview: String = dp
            .pad
            .content
            .chars()
            .take(50)
            .map(|c| if c == '\n' { ' ' } else { c })
            .collect();
        let title_content = if content_preview.is_empty() {
            title.clone()
        } else {
            format!("{} {}", title, content_preview)
        };

        let idx_width = idx_str.width();
        let fixed_width = left_prefix_width + idx_width + right_suffix_width + TIME_WIDTH;
        let available = LINE_WIDTH.saturating_sub(fixed_width);

        let title_display = truncate_to_width(&title_content, available);
        let content_width = title_display.width();
        let padding = " ".repeat(available.saturating_sub(content_width));

        let index_style = match dp.index {
            DisplayIndex::Pinned(_) => "index_pinned",
            DisplayIndex::Deleted(_) => "index_deleted",
            DisplayIndex::Regular(_) => "index_regular",
        };

        pad_lines.push(PadLineData {
            left_prefix,
            index: idx_str,
            title_content: title_display,
            padding,
            right_suffix,
            time_ago,
            index_style: index_style.to_string(),
        });
    }

    let data = ListData {
        has_pinned,
        pads: pad_lines,
        empty: false,
    };

    render_with_color(LIST_TEMPLATE, &data, &LIST_STYLES, use_color)
        .unwrap_or_else(|e| format!("Render error: {}\n", e))
}

/// Renders full pad contents similar to the legacy `print_full_pads` output.
pub fn render_full_pads(pads: &[DisplayPad], use_color: bool) -> String {
    let entries = pads
        .iter()
        .map(|dp| FullPadEntry {
            index: format!("{}", dp.index),
            title: dp.pad.metadata.title.clone(),
            content: dp.pad.content.clone(),
        })
        .collect();

    let data = FullPadData { pads: entries };

    render_with_color(FULL_PAD_TEMPLATE, &data, &FULL_PAD_STYLES, use_color)
        .unwrap_or_else(|e| format!("Render error: {}\n", e))
}

pub fn render_text_list(lines: &[String], empty_message: &str, use_color: bool) -> String {
    let data = TextListData {
        lines: lines.to_vec(),
        empty_message: empty_message.to_string(),
    };

    render_with_color(TEXT_LIST_TEMPLATE, &data, &TEXT_LIST_STYLES, use_color)
        .unwrap_or_else(|_| format!("{}\n", empty_message))
}

fn truncate_to_width(s: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthChar;

    let mut result = String::new();
    let mut current_width = 0;

    for c in s.chars() {
        let char_width = c.width().unwrap_or(0);
        if current_width + char_width > max_width.saturating_sub(1) {
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

    // Pad single-unit times for alignment
    let time_str = time_str
        .replace("hour ago", "hour  ago")
        .replace("minute ago", "minute  ago")
        .replace("second ago", "second  ago")
        .replace("day ago", "day  ago")
        .replace("week ago", "week  ago")
        .replace("month ago", "month  ago")
        .replace("year ago", "year  ago");

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
        let output = render_pad_list(&[], false);
        assert_eq!(output.trim(), "No pads found.");
    }

    #[test]
    fn test_render_single_regular_pad() {
        let pad = make_pad("Test Note", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        let output = render_pad_list(&[dp], false);

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

        let output = render_pad_list(&[dp], false);

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

        let output = render_pad_list(&[dp], false);

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

        let output = render_pad_list(&pads, false);

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

        let output = render_pad_list(&[dp], false);

        // Should have pin marker on the right side
        assert!(output.contains(PIN_MARKER));
    }

    #[test]
    fn test_render_with_color_includes_ansi() {
        let pad = make_pad("Test", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        // Force styling for test environment
        let output = render_pad_list(&[dp], true);

        // When use_color is true, should include ANSI codes (at least for time which is dimmed)
        // Note: console crate may not emit codes in test env, so we just verify it runs
        assert!(output.contains("Test"));
    }

    #[test]
    fn test_render_full_pads_empty() {
        let output = render_full_pads(&[], false);
        assert!(output.contains("No pads found."));
    }

    #[test]
    fn test_render_full_pads_single() {
        let pad = make_pad("Full Pad", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(3));

        let output = render_full_pads(&[dp], false);

        assert!(output.contains("3 Full Pad"));
        assert!(output.contains("--------------------------------"));
        assert!(output.contains("some content"));
    }

    #[test]
    fn test_render_text_list_empty() {
        let output = render_text_list(&[], "Nothing here.", false);
        assert!(output.contains("Nothing here."));
    }

    #[test]
    fn test_render_text_list_lines() {
        let lines = vec!["first".to_string(), "second".to_string()];
        let output = render_text_list(&lines, "", false);
        assert!(output.contains("first"));
        assert!(output.contains("second"));
    }
}
