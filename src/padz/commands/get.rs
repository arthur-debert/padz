use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::{index_pads, DisplayIndex, DisplayPad, MatchSegment, SearchMatch};
use crate::model::Scope;
use crate::store::DataStore;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PadStatusFilter {
    All,
    Active,
    Deleted,
    Pinned, // Only pinned? Or pinned active? Let's say pinned active.
}

#[derive(Debug, Clone)]
pub struct PadFilter {
    pub status: PadStatusFilter,
    pub search_term: Option<String>,
}

impl Default for PadFilter {
    fn default() -> Self {
        Self {
            status: PadStatusFilter::Active,
            search_term: None,
        }
    }
}

pub fn run<S: DataStore>(store: &S, scope: Scope, filter: PadFilter) -> Result<CmdResult> {
    // 1. Fetch relevant pads based on status to minimize processing
    // Currently store only has list_pads returning all.
    // If we want to optimize store access we would need store changes.
    // For now we fetch all and filter in memory as current implementation does.
    let pads = store.list_pads(scope)?;
    let indexed = index_pads(pads);

    let mut filtered: Vec<DisplayPad> = indexed
        .into_iter()
        .filter(|dp| match filter.status {
            PadStatusFilter::All => true,
            PadStatusFilter::Active => !matches!(dp.index, DisplayIndex::Deleted(_)),
            PadStatusFilter::Deleted => matches!(dp.index, DisplayIndex::Deleted(_)),
            PadStatusFilter::Pinned => matches!(dp.index, DisplayIndex::Pinned(_)),
        })
        .collect();

    // 2. Apply search if needed
    if let Some(term) = &filter.search_term {
        let term_lower = term.to_lowercase();
        let mut matches: Vec<(DisplayPad, u8)> = filtered
            .into_iter()
            .filter_map(|mut dp| {
                let mut search_matches = Vec::new();
                let mut score = 0;

                // Check title
                let title_lower = dp.pad.metadata.title.to_lowercase();
                if title_lower.contains(&term_lower) {
                    score += 10;
                    // For title, we just mark line 0 and full match segments
                    search_matches.push(SearchMatch {
                        line_number: 0,
                        segments: highlight_matches(&dp.pad.metadata.title, &term_lower),
                    });
                }

                // Check content
                // Split by logical lines, not just newlines, but let's stick to lines for now
                for (idx, line) in dp.pad.content.lines().enumerate() {
                    // Skip the first line as it typically duplicates the title
                    if idx == 0 {
                        continue;
                    }

                    let line_lower = line.to_lowercase();
                    if line_lower.contains(&term_lower) {
                        score += 5;
                        if search_matches.len() < 4 {
                            // Context extraction: 3 words before, 3 words after
                            // We'll simplify and use the whole line with highlighting for now,
                            // or implement the word context logic.
                            // Let's implement robust context extraction.
                            let segments = extract_context(line, &term_lower, 3);
                            search_matches.push(SearchMatch {
                                line_number: idx + 1, // 1-based for content
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

        // Sort by score then metadata
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

        filtered = matches.into_iter().map(|(dp, _)| dp).collect();
    }

    Ok(CmdResult::default().with_listed_pads(filtered))
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
/// Returns segments with "..." if truncated.
/// Extracts context around the first occurrence of `term` in `line`.
/// Returns segments with "..." if truncated.
fn extract_context(line: &str, term_lower: &str, context_words: usize) -> Vec<MatchSegment> {
    let line_lower = line.to_lowercase();
    let start_idx = match line_lower.find(term_lower) {
        Some(idx) => idx,
        None => return vec![MatchSegment::Plain(line.to_string())], // Should not happen if called correctly
    };

    let term_len = term_lower.len();
    let end_idx = start_idx + term_len;

    // Helper to identify word separators (whitespace or dots)
    let is_separator = |c: char| c.is_whitespace() || c == '.';

    // Find words before
    let pre_match = &line[..start_idx];
    let mut start_context_idx = 0;

    // Count words backwards
    let mut words_found = 0;
    let mut in_word = false;
    for (idx, c) in pre_match.char_indices().rev() {
        let is_sep = is_separator(c);
        if !is_sep && !in_word {
            // Entered a word
            words_found += 1;
            in_word = true;
        } else if is_sep && in_word {
            // Left a word
            in_word = false;
        }

        if words_found > context_words {
            // We found one too many words, so the start is the NEXT character
            start_context_idx = idx + c.len_utf8();
            break;
        }
    }

    // Find words after
    let post_match = &line[end_idx..];
    let mut end_context_idx = line.len(); // Default to end

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
            // We found one too many words, so the end is HERE (before this word starts)
            // Actually, we want to include the separator after the last allowed word?
            // Or cut right at the start of the excess word.
            // If we are at the start of N+1 word, idx is the start.
            end_context_idx = end_idx + idx;
            break;
        }
    }

    let mut segments = Vec::new();

    if start_context_idx > 0 {
        segments.push(MatchSegment::Plain("...".to_string()));
    }

    // Now highlighting inside the window [start_context_idx, end_context_idx]

    // Text before match in window
    if start_idx > start_context_idx {
        segments.push(MatchSegment::Plain(
            line[start_context_idx..start_idx].to_string(),
        ));
    }

    // The match itself
    segments.push(MatchSegment::Match(line[start_idx..end_idx].to_string()));

    // Text after match in window
    if end_context_idx > end_idx {
        segments.push(MatchSegment::Plain(
            line[end_idx..end_context_idx].to_string(),
        ));
    }

    if end_context_idx < line.len() {
        segments.push(MatchSegment::Plain("...".to_string()));
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, delete};
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn test_filters() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Active".into(), "".into()).unwrap();
        create::run(&mut store, Scope::Project, "Deleted".into(), "".into()).unwrap();

        // Delete the second one
        delete::run(&mut store, Scope::Project, &[DisplayIndex::Regular(1)]).unwrap();

        // 1. Test Active
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
            },
        )
        .unwrap();
        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Active");

        // 2. Test Deleted
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Deleted,
                search_term: None,
            },
        )
        .unwrap();
        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Deleted");

        // 3. Test All
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::All,
                search_term: None,
            },
        )
        .unwrap();
        assert_eq!(res.listed_pads.len(), 2);
    }

    #[test]
    fn test_search() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Foo".into(), "".into()).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Bar".into(),
            "contains foo".into(),
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: Some("foo".into()),
            },
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);
        // "Foo" title match (score 10) > "Bar" content match (score 5)
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Foo");

        // Verify context matches
        let matches_0 = res.listed_pads[0].matches.as_ref().unwrap();
        assert!(matches_0.iter().any(|m| m.line_number == 0)); // Title match

        let matches_1 = res.listed_pads[1].matches.as_ref().unwrap();
        assert!(matches_1.iter().any(|m| m.line_number == 3)); // Content match
    }

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
        // 3 words before: "two three four"
        // 3 words after: "five six seven"
        let segments = extract_context(line, "match", 3);

        // Expected: "...two three four ", "match", " five six seven..."
        // Or similar whitespace handling.

        // My implementation preserves spaces before match: " two three four "
        assert!(segments.len() >= 3);
        // Plain("..."), Plain(" two three four "), Match("match"), Plain(" five six seven "), Plain("...")

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
        assert!(!joined.contains("One")); // Should be truncated
        assert!(!joined.contains("eight")); // Should be truncated
    }
}
