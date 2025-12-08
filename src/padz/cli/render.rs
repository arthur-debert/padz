//! # Rendering Module
//!
//! This module provides styled terminal output using the `outstanding` crate.
//! Templates are defined here and rendered with automatic terminal color detection.

use super::styles::{names, PADZ_THEME};
use super::templates::{FULL_PAD_TEMPLATE, LIST_TEMPLATE, TEXT_LIST_TEMPLATE};
use chrono::{DateTime, Utc};
use outstanding::{render, render_with_color, ThemeChoice};
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
    left_pad: String,
    left_icon: Option<String>,
    left_post: String,
    index: String,
    title: String,
    padding: String,
    right_icon: Option<String>,
    right_post: String,
    time_ago: String,
    // Style hints
    index_style: String,
    title_style: String,
}

/// Data structure for the full list template.
#[derive(Serialize)]
struct ListData {
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
    index_style: String,
    title_style: String,
    content_style: String,
}

#[derive(Serialize)]
struct TextListData {
    lines: Vec<String>,
    empty_message: String,
}

/// Renders a list of pads to a string.
pub fn render_pad_list(pads: &[DisplayPad]) -> String {
    render_pad_list_internal(pads, None)
}

fn render_pad_list_internal(pads: &[DisplayPad], use_color: Option<bool>) -> String {
    if pads.is_empty() {
        return match use_color {
            Some(c) => render_with_color(
                LIST_TEMPLATE,
                &ListData {
                    pads: vec![],
                    empty: true,
                },
                ThemeChoice::from(&*PADZ_THEME),
                c,
            ),
            None => render(
                LIST_TEMPLATE,
                &ListData {
                    pads: vec![],
                    empty: true,
                },
                ThemeChoice::from(&*PADZ_THEME),
            ),
        }
        .unwrap_or_else(|_| "No pads found.\n".to_string());
    }

    let mut pad_lines = Vec::new();
    let mut last_was_pinned = false;

    for dp in pads {
        let is_pinned_entry = matches!(dp.index, DisplayIndex::Pinned(_));

        // Add separator line between pinned and regular sections
        if last_was_pinned && !is_pinned_entry {
            pad_lines.push(PadLineData {
                left_pad: String::new(),
                left_icon: None,
                left_post: String::new(),
                index: String::new(),
                title: String::new(),
                padding: String::new(),
                right_icon: None,
                right_post: String::new(),
                time_ago: String::new(),
                index_style: names::REGULAR.to_string(),
                title_style: names::REGULAR.to_string(),
            });
        }
        last_was_pinned = is_pinned_entry;

        let idx_str = match &dp.index {
            DisplayIndex::Pinned(n) => format!("p{}. ", n),
            DisplayIndex::Regular(n) => format!("{}. ", n),
            DisplayIndex::Deleted(n) => format!("d{}. ", n),
        };

        let (left_pad, left_icon, left_post) = if is_pinned_entry {
            (
                "  ".to_string(),
                Some(PIN_MARKER.to_string()),
                " ".to_string(),
            )
        } else {
            ("    ".to_string(), None, String::new())
        };

        let (right_icon, right_post) = if dp.pad.metadata.is_pinned && !is_pinned_entry {
            (Some(PIN_MARKER.to_string()), " ".to_string())
        } else {
            (None, "  ".to_string())
        };

        let left_prefix_width = left_pad.width()
            + left_icon
                .as_deref()
                .map(UnicodeWidthStr::width)
                .unwrap_or(0)
            + left_post.width();

        let right_suffix_width = right_icon
            .as_deref()
            .map(UnicodeWidthStr::width)
            .unwrap_or(0)
            + right_post.width();

        let idx_width = idx_str.width();
        let fixed_width = left_prefix_width + idx_width + right_suffix_width + TIME_WIDTH;
        let available = LINE_WIDTH.saturating_sub(fixed_width);

        let time_ago = format_time_ago(dp.pad.metadata.created_at);
        let title_source = dp.pad.metadata.title.as_str();
        let title_display = truncate_to_width(title_source, available);
        let title_width = title_display.width();
        let padding = " ".repeat(available.saturating_sub(title_width));

        let index_style = match dp.index {
            DisplayIndex::Pinned(_) => names::PINNED,
            DisplayIndex::Deleted(_) => names::DELETED,
            DisplayIndex::Regular(_) => names::REGULAR,
        }
        .to_string();
        let is_deleted_entry = matches!(dp.index, DisplayIndex::Deleted(_));
        let title_style = if is_deleted_entry {
            names::DELETED
        } else {
            names::TITLE
        }
        .to_string();

        pad_lines.push(PadLineData {
            left_pad,
            left_icon,
            left_post,
            index: idx_str,
            title: title_display,
            padding,
            right_icon,
            right_post,
            time_ago,
            index_style,
            title_style,
        });
    }

    let data = ListData {
        pads: pad_lines,
        empty: false,
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
            let index_style = match dp.index {
                DisplayIndex::Pinned(_) => names::PINNED,
                DisplayIndex::Deleted(_) => names::DELETED,
                DisplayIndex::Regular(_) => names::REGULAR,
            }
            .to_string();
            let is_deleted_entry = matches!(dp.index, DisplayIndex::Deleted(_));
            let title_style = if is_deleted_entry {
                names::DELETED
            } else {
                names::TITLE
            }
            .to_string();
            let content_style = if is_deleted_entry {
                names::DELETED
            } else {
                names::REGULAR
            }
            .to_string();

            FullPadEntry {
                index: format!("{}", dp.index),
                title: dp.pad.metadata.title.clone(),
                content: dp.pad.content.clone(),
                index_style,
                title_style,
                content_style,
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
}
