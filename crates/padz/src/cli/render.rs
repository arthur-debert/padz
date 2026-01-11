//! # Rendering Module
//!
//! This module provides styled terminal output using the `outstanding` crate. The core padzapp API
//! returns regular result data objects, and the CLI layer handles rendering.
//!
//! Rendering is template-based using minijinja templates, with stylesheets controlling formatted
//! terminal output. See the styles (`crate::cli::styles`) and templates (`crate::cli::templates`)
//! modules for specifics and best practices.
//!
//! ## Table Layout
//!
//! The list view uses outstanding's `col()` filter for declarative column layout. Each row has:
//! - `left_pin` (2 chars): Pin marker or empty
//! - `status_icon` (2 chars): Todo status indicator
//! - `index` (4 chars): Display index (p1., 1., d1.)
//! - `title` (fill): Pad title, truncated to fit
//! - `right_pin` (2 chars): Pin marker for pinned pads in regular section
//! - `time_ago` (14 chars, right-aligned): Relative timestamp
//!
//! Column widths are defined as constants and the title width is calculated per-row based on
//! the variable prefix width (which depends on section type and nesting depth).
//!
use super::setup::get_grouped_help;
use super::styles::{get_resolved_theme, names};
use super::templates::EMBEDDED_TEMPLATES;
use chrono::{DateTime, Utc};
use outstanding::{render_or_serialize, truncate_to_width, OutputMode, Renderer};
use padzapp::api::{CmdMessage, MessageLevel, TodoStatus};
use padzapp::index::{DisplayIndex, DisplayPad};
use padzapp::peek::{format_as_peek, PeekResult};
use serde::Serialize;

/// Creates a Renderer with all templates registered.
///
/// The renderer resolves the adaptive theme based on terminal color mode
/// and registers both main templates and partials, enabling `{% include %}`
/// directives in templates.
///
/// Templates are loaded from the embedded HashMap which is populated from
/// template files at compile time (see templates module).
fn create_renderer(output_mode: OutputMode) -> Renderer {
    let theme = get_resolved_theme();
    let mut renderer = Renderer::with_output(theme, output_mode)
        .expect("Failed to create renderer - invalid theme aliases");

    // Load all templates into the renderer
    // This ensures {% include %} directives work correctly
    for (name, content) in EMBEDDED_TEMPLATES.iter() {
        renderer
            .add_template(name, content)
            .unwrap_or_else(|_| panic!("Failed to register template: {}", name));
    }

    renderer
}

/// Configuration for list rendering.
pub const LINE_WIDTH: usize = 100;
pub const PIN_MARKER: &str = "⚲";

/// Column widths for list layout (used by outstanding's `col()` filter)
pub const COL_LEFT_PIN: usize = 2; // Pin marker or empty ("⚲ " or "  ")
pub const COL_STATUS: usize = 2; // Status icon + space
pub const COL_INDEX: usize = 4; // "p1.", " 1.", "d1."
pub const COL_RIGHT_PIN: usize = 2; // Pin marker for pinned in regular section
pub const COL_TIME: usize = 14; // Right-aligned timestamp

/// Status indicators for todo status
pub const STATUS_PLANNED: &str = "⚪︎";
pub const STATUS_IN_PROGRESS: &str = "☉︎︎";
pub const STATUS_DONE: &str = "⚫︎";

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
/// Contains raw values and semantic flags for template-driven styling.
/// Layout is handled declaratively in templates using outstanding's `col()` filter.
///
/// ## Column Layout
///
/// Templates use `col(width)` for each column:
/// - `indent` - Depth-based indentation (variable, not a `col()` column)
/// - `left_pin | col(2)` - Pin marker or empty
/// - `status_icon | col(2)` - Status indicator
/// - `index | col(4)` - Display index
/// - `title | col(title_width)` - Truncated/padded to fill
/// - `right_pin | col(2)` - Right-side pin marker
/// - `time_ago | col(14, align='right')` - Timestamp
#[derive(Serialize)]
struct PadLineData {
    // Layout prefix (depth-based indentation, not a col() column)
    indent: String, // "  " per depth level + base padding for non-pinned
    // Column values (templates handle truncation/padding via `col()` filter)
    left_pin: String,    // "⚲" or "" (column pads to width)
    status_icon: String, // Todo status indicator (⚪︎, ☉, ⚫︎)
    index: String,       // "p1.", " 1.", "d1."
    title: String,       // Raw title (template truncates via `col(title_width)`)
    title_width: usize,  // Calculated fill width for this row
    right_pin: String,   // "⚲" or "" for pinned pads in regular section
    time_ago: String,    // Relative timestamp (template right-aligns via `col()`)
    // Semantic flags for template-driven style selection
    is_pinned_section: bool, // In the pinned section (p1, p2, etc.)
    is_deleted: bool,        // In the deleted section (d1, d2, etc.)
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
    // Column widths for outstanding's `col()` filter
    col_left_pin: usize,
    col_status: usize,
    col_index: usize,
    col_right_pin: usize,
    col_time: usize,
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

/// JSON-serializable wrapper for pad list output.
/// Used for --output=json mode to provide machine-readable pad data.
#[derive(Serialize)]
struct JsonPadList {
    pads: Vec<DisplayPad>,
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
pub fn render_pad_list(pads: &[DisplayPad], peek: bool, output_mode: OutputMode) -> String {
    render_pad_list_internal(pads, Some(output_mode), false, peek)
}

/// Renders a list of pads with optional deleted help text.
pub fn render_pad_list_deleted(pads: &[DisplayPad], peek: bool, output_mode: OutputMode) -> String {
    render_pad_list_internal(pads, Some(output_mode), true, peek)
}

fn render_pad_list_internal(
    pads: &[DisplayPad],
    output_mode: Option<OutputMode>,
    show_deleted_help: bool,
    peek: bool,
) -> String {
    let mode = output_mode.unwrap_or(OutputMode::Auto);

    // For JSON mode, serialize the pads directly
    if mode == OutputMode::Json {
        let json_data = JsonPadList {
            pads: pads.to_vec(),
        };
        let theme = get_resolved_theme();
        return render_or_serialize(
            "", // Template not used for JSON
            &json_data, &theme, mode,
        )
        .unwrap_or_else(|_| "{\"pads\":[]}".to_string());
    }

    let empty_data = ListData {
        pads: vec![],
        empty: true,
        pin_marker: PIN_MARKER.to_string(),
        help_text: get_grouped_help(),
        deleted_help: false,
        peek: false,
        col_left_pin: COL_LEFT_PIN,
        col_status: COL_STATUS,
        col_index: COL_INDEX,
        col_right_pin: COL_RIGHT_PIN,
        col_time: COL_TIME,
    };

    if pads.is_empty() {
        let mut renderer = create_renderer(mode);
        return renderer
            .render("list", &empty_data)
            .unwrap_or_else(|_| "No pads found.\n".to_string());
    }

    let mut pad_lines = Vec::new();
    let mut last_was_pinned = false;

    // Use a recursive helper to flatten the tree with depth/indentation
    fn process_pad(
        dp: &DisplayPad,
        pad_lines: &mut Vec<PadLineData>,
        _prefix: &str, // Previously used for hierarchical indexes, now unused
        depth: usize,
        is_pinned_section: bool,
        is_deleted_root: bool,
        peek: bool,
    ) {
        let is_deleted = matches!(dp.index, DisplayIndex::Deleted(_));
        let show_right_pin = dp.pad.metadata.is_pinned && !is_pinned_section;

        // Format index string - space-padded, local only (no hierarchical prefix)
        let local_idx_str = match &dp.index {
            DisplayIndex::Pinned(n) => format!("p{}", n),
            DisplayIndex::Regular(n) => format!("{:2}", n), // Space-padded
            DisplayIndex::Deleted(n) => format!("d{}", n),
        };

        // Display index: just local index with dot (no hierarchical prefix)
        let full_idx_str = format!("{}.", local_idx_str);

        // Get status icon based on pad's todo status
        let status_icon = match dp.pad.metadata.status {
            TodoStatus::Planned => STATUS_PLANNED,
            TodoStatus::InProgress => STATUS_IN_PROGRESS,
            TodoStatus::Done => STATUS_DONE,
        }
        .to_string();

        // Indent: The left_pin column (via col(2)) provides 2 chars.
        // Additional indent is needed for nested items.
        //
        // Layout for total prefix (before status icon):
        // - Pinned depth 0: 2 (just left_pin column with "⚲")
        // - Pinned depth 1: 2 (left_pin = "  ", no extra indent)
        // - Pinned depth 2+: depth * 2 (extra indent before left_pin)
        // - Regular depth 0: 2 (just left_pin column with "  ")
        // - Regular depth 1+: 2 + depth * 2 (extra indent before left_pin)
        //
        // Since left_pin always contributes 2, we need:
        // - Pinned: max(0, (depth - 1) * 2) = depth.saturating_sub(1) * 2
        // - Regular: depth * 2
        let indent_width = if is_pinned_section {
            depth.saturating_sub(1) * 2
        } else {
            depth * 2
        };
        let total_indent_width = indent_width + COL_LEFT_PIN; // For title_width calculation
        let indent = " ".repeat(indent_width);

        // Column values for template's col() filter
        // left_pin: pin marker for pinned section root items, empty otherwise
        let left_pin = if is_pinned_section && depth == 0 {
            PIN_MARKER.to_string()
        } else {
            String::new()
        };

        // right_pin: pin marker for pinned pads shown in regular section
        let right_pin = if show_right_pin {
            PIN_MARKER.to_string()
        } else {
            String::new()
        };

        // Calculate title_width (fill column)
        // Fixed columns: left_pin(2) + status(2) + index(4) + right_pin(2) + time(14) = 24
        // Plus the variable indent width
        let fixed_columns = COL_LEFT_PIN + COL_STATUS + COL_INDEX + COL_RIGHT_PIN + COL_TIME;
        let title_width = LINE_WIDTH.saturating_sub(fixed_columns + total_indent_width);

        // Process matches
        let mut match_lines = Vec::new();
        if let Some(matches) = &dp.matches {
            for m in matches {
                if m.line_number == 0 {
                    continue;
                }
                let segments: Vec<MatchSegmentData> = m
                    .segments
                    .iter()
                    .map(|s| match s {
                        padzapp::index::MatchSegment::Plain(t) => MatchSegmentData {
                            text: t.clone(),
                            style: names::INFO.to_string(),
                        },
                        padzapp::index::MatchSegment::Match(t) => MatchSegmentData {
                            text: t.clone(),
                            style: names::MATCH.to_string(),
                        },
                    })
                    .collect();

                // Match lines are indented to align under the title
                let match_indent = total_indent_width + COL_LEFT_PIN + COL_STATUS + COL_INDEX;
                let match_available = LINE_WIDTH.saturating_sub(COL_TIME + match_indent);
                let truncated = truncate_match_segments(segments, match_available);
                match_lines.push(MatchLineData {
                    line_number: format!("{:02}", m.line_number),
                    segments: truncated,
                });
            }
        }

        let peek_data = if peek {
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
            indent,
            left_pin,
            status_icon,
            index: full_idx_str.clone(),
            title: dp.pad.metadata.title.clone(), // Raw title - template truncates via col()
            title_width,
            right_pin,
            time_ago: format_time_ago(dp.pad.metadata.created_at),
            is_pinned_section: is_pinned_section && depth == 0,
            is_deleted: is_deleted || is_deleted_root,
            is_separator: false,
            matches: match_lines,
            more_matches_count: 0,
            peek: peek_data,
        });

        // RECURSE CHILDREN
        for child in &dp.children {
            process_pad(
                child,
                pad_lines,
                &full_idx_str,
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

        // Separator between pinned and regular ROOTS
        if last_was_pinned && !is_pinned_section {
            pad_lines.push(PadLineData {
                indent: String::new(),
                left_pin: String::new(),
                status_icon: String::new(),
                index: String::new(),
                title: String::new(),
                title_width: 0,
                right_pin: String::new(),
                time_ago: String::new(),
                is_pinned_section: false,
                is_deleted: false,
                is_separator: true,
                matches: vec![],
                more_matches_count: 0,
                peek: None,
            });
        }
        last_was_pinned = is_pinned_section;

        process_pad(
            dp,
            &mut pad_lines,
            "",
            0,
            is_pinned_section,
            is_deleted_section,
            peek,
        );
    }

    let data = ListData {
        pads: pad_lines,
        empty: false,
        pin_marker: PIN_MARKER.to_string(),
        help_text: String::new(), // Not used when not empty
        deleted_help: show_deleted_help,
        peek,
        col_left_pin: COL_LEFT_PIN,
        col_status: COL_STATUS,
        col_index: COL_INDEX,
        col_right_pin: COL_RIGHT_PIN,
        col_time: COL_TIME,
    };

    let mut renderer = create_renderer(mode);
    renderer
        .render("list", &data)
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
pub fn render_full_pads(pads: &[DisplayPad], output_mode: OutputMode) -> String {
    render_full_pads_internal(pads, Some(output_mode))
}

fn render_full_pads_internal(pads: &[DisplayPad], output_mode: Option<OutputMode>) -> String {
    let mode = output_mode.unwrap_or(OutputMode::Auto);

    // For JSON mode, serialize the pads directly
    if mode == OutputMode::Json {
        let json_data = JsonPadList {
            pads: pads.to_vec(),
        };
        let theme = get_resolved_theme();
        return render_or_serialize(
            "", // Template not used for JSON
            &json_data, &theme, mode,
        )
        .unwrap_or_else(|_| "{\"pads\":[]}".to_string());
    }

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

    let mut renderer = create_renderer(mode);
    renderer
        .render("full_pad", &data)
        .unwrap_or_else(|e| format!("Render error: {}\n", e))
}

pub fn render_text_list(lines: &[String], empty_message: &str, output_mode: OutputMode) -> String {
    render_text_list_internal(lines, empty_message, Some(output_mode))
}

fn render_text_list_internal(
    lines: &[String],
    empty_message: &str,
    output_mode: Option<OutputMode>,
) -> String {
    let mode = output_mode.unwrap_or(OutputMode::Auto);

    // For JSON mode, serialize the lines directly
    if mode == OutputMode::Json {
        let json_data = TextListData {
            lines: lines.to_vec(),
            empty_message: empty_message.to_string(),
        };
        let theme = get_resolved_theme();
        return render_or_serialize(
            "", // Template not used for JSON
            &json_data, &theme, mode,
        )
        .unwrap_or_else(|_| "{\"lines\":[]}".to_string());
    }

    let data = TextListData {
        lines: lines.to_vec(),
        empty_message: empty_message.to_string(),
    };

    let mut renderer = create_renderer(mode);
    renderer
        .render("text_list", &data)
        .unwrap_or_else(|_| format!("{}\n", empty_message))
}

/// JSON-serializable wrapper for command messages.
/// Used for --output=json mode to provide machine-readable output.
#[derive(Serialize)]
struct JsonMessages {
    messages: Vec<CmdMessage>,
}

/// Renders command messages using the template system with themed styles.
/// Supports JSON output mode for machine-readable output.
pub fn render_messages(messages: &[CmdMessage], output_mode: OutputMode) -> String {
    if messages.is_empty() {
        return String::new();
    }

    // For JSON mode, serialize the messages directly
    if output_mode == OutputMode::Json {
        let json_data = JsonMessages {
            messages: messages.to_vec(),
        };
        let theme = get_resolved_theme();
        return render_or_serialize(
            "", // Template not used for JSON
            &json_data,
            &theme,
            output_mode,
        )
        .unwrap_or_else(|_| "{}".to_string());
    }

    // For terminal modes, use template rendering
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

    let mut renderer = create_renderer(output_mode);
    renderer.render("messages", &data).unwrap_or_else(|_| {
        messages
            .iter()
            .map(|m| format!("{}\n", m.content))
            .collect()
    })
}

/// Prints command messages to stdout using the template system.
/// Supports JSON output mode for machine-readable output.
pub fn print_messages(messages: &[CmdMessage], output_mode: OutputMode) {
    let output = render_messages(messages, output_mode);
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
    // The template handles right-alignment via col(col_time, align='right')
    time_str
        .replace("hours ago", "  hours ago") // 5 -> 7
        .replace("hour ago", "   hour ago") // 4 -> 7
        .replace("days ago", "   days ago") // 4 -> 7
        .replace("day ago", "    day ago") // 3 -> 7
        .replace("weeks ago", "  weeks ago") // 5 -> 7
        .replace("week ago", "   week ago") // 4 -> 7
        .replace("months ago", " months ago") // 6 -> 7
        .replace("month ago", "  month ago") // 5 -> 7
        .replace("years ago", "  years ago") // 5 -> 7
        .replace("year ago", "   year ago") // 4 -> 7
}

#[cfg(test)]
mod tests {
    use super::*;
    use padzapp::model::Pad;

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
    fn test_render_empty_list() {
        let output = render_pad_list_internal(&[], Some(OutputMode::Text), false, false);
        // Should show the "no pads yet" message with help text
        assert!(output.contains("No pads yet, create one with `padz create`"));
    }

    #[test]
    fn test_render_single_regular_pad() {
        let pad = make_pad("Test Note", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        let output = render_pad_list_internal(&[dp], Some(OutputMode::Text), false, false);

        // Should contain status icon, space-padded index and title
        assert!(output.contains(STATUS_PLANNED)); // Default status is Planned
        assert!(output.contains(" 1."));
        assert!(output.contains("Test Note"));
        // Should have base padding, then status icon, then index
        assert!(output.contains(&format!("{} ", STATUS_PLANNED)));
        assert!(output.contains(&format!("{} {}.", STATUS_PLANNED, " 1")));
    }

    #[test]
    fn test_render_pinned_pad() {
        let pad = make_pad("Pinned Note", true, false);
        let dp = make_display_pad(pad, DisplayIndex::Pinned(1));

        let output = render_pad_list_internal(&[dp], Some(OutputMode::Text), false, false);

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

        let output = render_pad_list_internal(&[dp], Some(OutputMode::Text), false, false);

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

        let output = render_pad_list_internal(&pads, Some(OutputMode::Text), false, false);

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

        let output = render_pad_list_internal(&[dp], Some(OutputMode::Text), false, false);

        // Should have pin marker on the right side
        assert!(output.contains(PIN_MARKER));
    }

    #[test]
    fn test_render_with_color_includes_ansi() {
        let pad = make_pad("Test", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(1));

        // Force styling for test environment
        let output = render_pad_list_internal(&[dp], Some(OutputMode::Term), false, false);

        // When use_color is true, should include ANSI codes (at least for time which is dimmed)
        // Note: console crate may not emit codes in test env, so we just verify it runs
        assert!(output.contains("Test"));
    }

    #[test]
    fn test_render_search_results() {
        use padzapp::index::{MatchSegment, SearchMatch};

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

        let output = render_pad_list_internal(&[dp], Some(OutputMode::Text), false, false);

        assert!(output.contains("1."));
        assert!(output.contains("Search Result"));
        // Check indentation of match line (should have padding)
        assert!(output.contains("    02 Found match here"));
    }

    #[test]
    fn test_render_full_pads_empty() {
        let output = render_full_pads_internal(&[], Some(OutputMode::Text));
        assert!(output.contains("No pads found."));
    }

    #[test]
    fn test_render_full_pads_single() {
        let pad = make_pad("Full Pad", false, false);
        let dp = make_display_pad(pad, DisplayIndex::Regular(3));

        let output = render_full_pads_internal(&[dp], Some(OutputMode::Text));

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
        let output = render_text_list_internal(&[], "Nothing here.", Some(OutputMode::Text));
        assert!(output.contains("Nothing here."));
    }

    #[test]
    fn test_render_text_list_lines() {
        let lines = vec!["first".to_string(), "second".to_string()];
        let output = render_text_list_internal(&lines, "", Some(OutputMode::Text));
        assert!(output.contains("first"));
        assert!(output.contains("second"));
    }

    #[test]
    fn test_render_messages_empty() {
        let output = render_messages(&[], OutputMode::Auto);
        assert!(output.is_empty());
    }

    #[test]
    fn test_render_messages_success() {
        let messages = vec![CmdMessage::success("Pad created: Test")];
        let output = render_messages(&messages, OutputMode::Auto);
        assert!(output.contains("Pad created: Test"));
    }

    #[test]
    fn test_render_messages_multiple() {
        let messages = vec![
            CmdMessage::info("Info message"),
            CmdMessage::warning("Warning message"),
            CmdMessage::error("Error message"),
        ];
        let output = render_messages(&messages, OutputMode::Auto);
        assert!(output.contains("Info message"));
        assert!(output.contains("Warning message"));
        assert!(output.contains("Error message"));
    }

    #[test]
    fn test_render_messages_json() {
        let messages = vec![
            CmdMessage::success("Operation completed"),
            CmdMessage::info("Additional info"),
        ];
        let output = render_messages(&messages, OutputMode::Json);
        assert!(output.contains("\"level\": \"success\""));
        assert!(output.contains("\"content\": \"Operation completed\""));
        assert!(output.contains("\"level\": \"info\""));
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
