use crate::index::{DisplayPad, MatchSegment, SearchMatch};

pub(super) fn apply_search(pads: Vec<DisplayPad>, term: &str) -> Vec<DisplayPad> {
    let term_lower = term.to_lowercase();
    let mut matches: Vec<(DisplayPad, u8)> = pads
        .into_iter()
        .filter_map(|mut dp| {
            let mut search_matches = Vec::new();
            let mut score = 0;

            // Check title
            let title_lower = dp.pad.metadata.title.to_lowercase();
            if title_lower.contains(&term_lower) {
                score += 10;
                search_matches.push(SearchMatch {
                    line_number: 0,
                    segments: highlight_matches(&dp.pad.metadata.title, &term_lower),
                });
            }

            // Check content lines (skip first line, which duplicates the title)
            for (idx, line) in dp.pad.content.lines().enumerate() {
                if idx == 0 {
                    continue;
                }

                let line_lower = line.to_lowercase();
                if line_lower.contains(&term_lower) {
                    score += 5;
                    if search_matches.len() < 4 {
                        let segments = extract_context(line, &term_lower, 3);
                        search_matches.push(SearchMatch {
                            line_number: idx + 1,
                            segments,
                        });
                    }
                }
            }

            if score > 0 {
                dp.matches = Some(search_matches);
                Some((dp, score))
            } else {
                None
            }
        })
        .collect();

    matches.sort_by(
        |(a, score_a), (b, score_b)| match score_a.cmp(score_b).reverse() {
            std::cmp::Ordering::Equal => {
                let len_a = a.pad.metadata.title.len();
                let len_b = b.pad.metadata.title.len();
                match len_a.cmp(&len_b) {
                    std::cmp::Ordering::Equal => {
                        a.pad.metadata.created_at.cmp(&b.pad.metadata.created_at)
                    }
                    ord => ord,
                }
            }
            ord => ord,
        },
    );

    matches.into_iter().map(|(dp, _)| dp).collect()
}

/// Highlights occurrences of `term` in `text` (case-insensitive).
fn highlight_matches(text: &str, term_lower: &str) -> Vec<MatchSegment> {
    let mut segments = Vec::new();
    let text_lower = text.to_lowercase();
    let term_len = term_lower.len();
    let mut last_idx = 0;

    for (start_idx, _) in text_lower.match_indices(term_lower) {
        if start_idx > last_idx {
            segments.push(MatchSegment::Plain(text[last_idx..start_idx].to_string()));
        }
        segments.push(MatchSegment::Match(
            text[start_idx..start_idx + term_len].to_string(),
        ));
        last_idx = start_idx + term_len;
    }

    if last_idx < text.len() {
        segments.push(MatchSegment::Plain(text[last_idx..].to_string()));
    }

    segments
}

/// Extracts context around the first occurrence of `term` in `line`.
/// Returns segments with "…" if truncated.
fn extract_context(line: &str, term_lower: &str, context_words: usize) -> Vec<MatchSegment> {
    let line_lower = line.to_lowercase();
    let start_idx = match line_lower.find(term_lower) {
        Some(idx) => idx,
        None => return vec![MatchSegment::Plain(line.to_string())],
    };

    let term_len = term_lower.len();
    let end_idx = start_idx + term_len;

    let is_separator = |c: char| c.is_whitespace() || c == '.';

    // Find words before
    let pre_match = &line[..start_idx];
    let mut start_context_idx = 0;

    let mut words_found = 0;
    let mut in_word = false;
    for (idx, c) in pre_match.char_indices().rev() {
        let is_sep = is_separator(c);
        if !is_sep && !in_word {
            words_found += 1;
            in_word = true;
        } else if is_sep && in_word {
            in_word = false;
        }

        if words_found > context_words {
            start_context_idx = idx + c.len_utf8();
            break;
        }
    }

    // Find words after
    let post_match = &line[end_idx..];
    let mut end_context_idx = line.len();

    words_found = 0;
    in_word = false;
    for (idx, c) in post_match.char_indices() {
        let is_sep = is_separator(c);
        if !is_sep && !in_word {
            words_found += 1;
            in_word = true;
        } else if is_sep && in_word {
            in_word = false;
        }

        if words_found > context_words {
            end_context_idx = end_idx + idx;
            break;
        }
    }

    let mut segments = Vec::new();

    if start_context_idx > 0 {
        segments.push(MatchSegment::Plain("…".to_string()));
    }

    if start_idx > start_context_idx {
        segments.push(MatchSegment::Plain(
            line[start_context_idx..start_idx].to_string(),
        ));
    }

    segments.push(MatchSegment::Match(line[start_idx..end_idx].to_string()));

    if end_context_idx > end_idx {
        segments.push(MatchSegment::Plain(
            line[end_idx..end_context_idx].to_string(),
        ));
    }

    if end_context_idx < line.len() {
        segments.push(MatchSegment::Plain("…".to_string()));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_matches() {
        let text = "Hello World";
        let segments = highlight_matches(text, "world");
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], MatchSegment::Plain("Hello ".to_string()));
        assert_eq!(segments[1], MatchSegment::Match("World".to_string()));
    }

    #[test]
    fn test_extract_context() {
        let line = "One two three four match five six seven eight";
        let segments = extract_context(line, "match", 3);

        assert!(segments.len() >= 3);

        let joined: String = segments
            .iter()
            .map(|s| match s {
                MatchSegment::Plain(t) => t.as_str(),
                MatchSegment::Match(t) => t.as_str(),
            })
            .collect();

        assert!(joined.contains("match"));
        assert!(joined.contains("two three four"));
        assert!(joined.contains("five six seven"));
        assert!(!joined.contains("One"));
        assert!(!joined.contains("eight"));
    }
}
