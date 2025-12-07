use chrono::{DateTime, Utc};
use colored::Colorize;
use padz::api::{CmdMessage, MessageLevel};
use padz::index::{DisplayIndex, DisplayPad};
use timeago::Formatter;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const LINE_WIDTH: usize = 100;
const TIME_WIDTH: usize = 14;
const PIN_MARKER: &str = "⚲";

pub(super) fn print_messages(messages: &[CmdMessage]) {
    for message in messages {
        match message.level {
            MessageLevel::Info => println!("{}", message.content.dimmed()),
            MessageLevel::Success => println!("{}", message.content.green()),
            MessageLevel::Warning => println!("{}", message.content.yellow()),
            MessageLevel::Error => println!("{}", message.content.red()),
        }
    }
}

pub(super) fn print_full_pads(pads: &[DisplayPad]) {
    for (i, dp) in pads.iter().enumerate() {
        if i > 0 {
            println!("\n================================\n");
        }
        println!(
            "{} {}",
            dp.index.to_string().yellow(),
            dp.pad.metadata.title.bold()
        );
        println!("--------------------------------");
        println!("{}", dp.pad.content);
    }
}

pub(super) fn print_pads(pads: &[DisplayPad]) {
    if pads.is_empty() {
        println!("No pads found.");
        return;
    }

    let has_pinned = pads
        .iter()
        .any(|dp| matches!(dp.index, DisplayIndex::Pinned(_)));
    if has_pinned {
        println!();
    }

    let mut last_was_pinned = false;
    for dp in pads {
        let is_pinned_entry = matches!(dp.index, DisplayIndex::Pinned(_));

        if last_was_pinned && !is_pinned_entry {
            println!();
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

        let title_display: String = truncate_to_width(&title_content, available);

        let content_width = title_display.width();
        let padding = available.saturating_sub(content_width);

        let idx_colored = match dp.index {
            DisplayIndex::Pinned(_) => idx_str.yellow(),
            DisplayIndex::Deleted(_) => idx_str.red(),
            DisplayIndex::Regular(_) => idx_str.normal(),
        };

        let time_colored = time_ago.dimmed();

        println!(
            "{}{}{}{}{}{}",
            left_prefix,
            idx_colored,
            title_display,
            " ".repeat(padding),
            right_suffix,
            time_colored
        );
    }
}

fn truncate_to_width(s: &str, max_width: usize) -> String {
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

    let formatter = Formatter::new();
    let time_str = formatter.convert(duration.to_std().unwrap_or_default());

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
