//! # Peek Preview Logic
//!
//! This module handles the formatting of pad content for "peek" views.
//! It truncates content based on configurable line limits while stripping blank lines.

use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct PeekResult {
    pub opening_lines: String,
    pub truncated_count: Option<usize>,
    pub closing_lines: Option<String>,
}

/// Formats raw pad content into a peek view.
///
/// Rules:
/// 1. Blank lines are ignored (stripped).
/// 2. If content lines <= (peek_line_num * 2) + 3, show full content (no truncation).
/// 3. Otherwise show:
///    - Opening lines (up to peek_line_num)
///    - Truncated count
///    - Closing lines (up to peek_line_num)
pub fn format_as_peek(raw_content: &str, peek_line_num: usize) -> PeekResult {
    // 1. Filter out blank lines
    let non_blank_lines: Vec<&str> = raw_content
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();

    let total_lines = non_blank_lines.len();

    // 2. Calculate threshold: (peek_line_num * 2) + 3
    let threshold = (peek_line_num * 2) + 3;

    if total_lines <= threshold {
        // No truncation needed
        return PeekResult {
            opening_lines: non_blank_lines.join("\n"),
            truncated_count: None,
            closing_lines: None,
        };
    }

    // 3. Truncate
    let opening = non_blank_lines[..peek_line_num].join("\n");
    let closing = non_blank_lines[total_lines - peek_line_num..].join("\n");
    let truncated_cnt = total_lines - (peek_line_num * 2);

    PeekResult {
        opening_lines: opening,
        truncated_count: Some(truncated_cnt),
        closing_lines: Some(closing),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peek_empty() {
        let res = format_as_peek("", 3);
        assert_eq!(res.opening_lines, "");
        assert_eq!(res.truncated_count, None);
        assert_eq!(res.closing_lines, None);
    }

    #[test]
    fn test_peek_short_no_truncation() {
        // threshold for 3 is (3*2)+3 = 9.
        // 5 lines < 9.
        let content = "One\nTwo\nThree\nFour\nFive";
        let res = format_as_peek(content, 3);
        assert_eq!(res.opening_lines, "One\nTwo\nThree\nFour\nFive");
        assert_eq!(res.truncated_count, None);
        assert_eq!(res.closing_lines, None);
    }

    #[test]
    fn test_peek_strips_blanks() {
        let content = "One\n\nTwo\n   \nThree";
        let res = format_as_peek(content, 3);
        assert_eq!(res.opening_lines, "One\nTwo\nThree");
        assert_eq!(res.truncated_count, None);
    }

    #[test]
    fn test_peek_exact_threshold() {
        // Threshold 9. Input 9 lines.
        let lines: Vec<String> = (1..=9).map(|i| i.to_string()).collect();
        let content = lines.join("\n");
        let res = format_as_peek(&content, 3);
        assert_eq!(res.opening_lines, content); // Joined lines
        assert_eq!(res.truncated_count, None);
    }

    #[test]
    fn test_peek_truncation() {
        // Threshold 9. Input 10 lines.
        // peek=3. opening=3, closing=3. truncated=4 (Wait: 10 - 6 = 4).
        let lines: Vec<String> = (1..=10).map(|i| i.to_string()).collect();
        let content = lines.join("\n");
        let res = format_as_peek(&content, 3);

        assert_eq!(res.opening_lines, "1\n2\n3");
        assert_eq!(res.truncated_count, Some(4));
        assert_eq!(res.closing_lines, Some("8\n9\n10".to_string()));
    }
}
