use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::{index_pads, DisplayIndex, DisplayPad, MatchSegment, SearchMatch};
use crate::model::{Scope, TodoStatus};
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
    /// Filter by todo status. None means show all (no filtering by todo status).
    pub todo_status: Option<TodoStatus>,
}

impl Default for PadFilter {
    fn default() -> Self {
        Self {
            status: PadStatusFilter::Active,
            search_term: None,
            todo_status: None, // Show all todo statuses by default
        }
    }
}

/// Recursively filters the tree based on status.
///
/// Filtering rules:
/// - **Active**: Show non-deleted pads. Children are recursively filtered (only non-deleted).
/// - **Deleted**: Show deleted pads with ALL their children (children aren't marked deleted
///   but are visible under their deleted parent per spec: "unless looking at deleted items").
/// - **Pinned**: Show pinned pads. Children are recursively filtered for pinned only.
/// - **All**: Show everything, no filtering.
fn filter_tree(pads: Vec<DisplayPad>, status: PadStatusFilter) -> Vec<DisplayPad> {
    pads.into_iter()
        .filter_map(|dp| filter_pad_recursive(dp, status))
        .collect()
}

fn filter_pad_recursive(mut dp: DisplayPad, status: PadStatusFilter) -> Option<DisplayPad> {
    let dominated = matches_status(&dp.index, status);

    if !dominated {
        return None;
    }

    // For Deleted status, show ALL children (they inherit visibility from deleted parent)
    // For other statuses, recursively filter children
    if status == PadStatusFilter::Deleted {
        // Children of a deleted pad are shown as-is (no further filtering)
        // But we still need to recurse to filter THEIR children if any are deleted
        dp.children = dp
            .children
            .into_iter()
            .map(filter_children_under_deleted)
            .collect();
    } else {
        dp.children = dp
            .children
            .into_iter()
            .filter_map(|child| filter_pad_recursive(child, status))
            .collect();
    }

    Some(dp)
}

/// When viewing deleted pads, children of a deleted parent are shown.
/// Those children might have their own children that need filtering.
fn filter_children_under_deleted(mut dp: DisplayPad) -> DisplayPad {
    // Show this child (regardless of its own deleted status since parent is deleted)
    // But recursively process its children
    dp.children = dp
        .children
        .into_iter()
        .map(filter_children_under_deleted)
        .collect();
    dp
}

/// Recursively filters the tree based on todo status.
/// Returns pads that match the specified todo status, preserving hierarchy.
fn filter_by_todo_status(pads: Vec<DisplayPad>, todo_status: TodoStatus) -> Vec<DisplayPad> {
    pads.into_iter()
        .filter_map(|dp| filter_pad_by_todo_status(dp, todo_status))
        .collect()
}

fn filter_pad_by_todo_status(mut dp: DisplayPad, todo_status: TodoStatus) -> Option<DisplayPad> {
    // First, recursively filter children
    dp.children = dp
        .children
        .into_iter()
        .filter_map(|child| filter_pad_by_todo_status(child, todo_status))
        .collect();

    // Include this pad if it matches the todo status
    if dp.pad.metadata.status == todo_status {
        Some(dp)
    } else {
        None
    }
}

fn matches_status(index: &DisplayIndex, status: PadStatusFilter) -> bool {
    match status {
        PadStatusFilter::All => true,
        PadStatusFilter::Active => !matches!(index, DisplayIndex::Deleted(_)),
        PadStatusFilter::Deleted => matches!(index, DisplayIndex::Deleted(_)),
        PadStatusFilter::Pinned => matches!(index, DisplayIndex::Pinned(_)),
    }
}

pub fn run<S: DataStore>(store: &S, scope: Scope, filter: PadFilter) -> Result<CmdResult> {
    let pads = store.list_pads(scope)?;
    let indexed = index_pads(pads);

    // 1. Filter by deletion status (Active/Deleted/Pinned)
    let mut filtered: Vec<DisplayPad> = filter_tree(indexed, filter.status);

    // 2. Filter by todo status if specified
    if let Some(todo_status) = filter.todo_status {
        filtered = filter_by_todo_status(filtered, todo_status);
    }

    // 3. Apply search if needed
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
    use crate::index::PadSelector;
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn test_filters() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Active".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Deleted".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Delete the second one
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // 1. Test Active
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: None,
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
                todo_status: None,
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
                todo_status: None,
            },
        )
        .unwrap();
        assert_eq!(res.listed_pads.len(), 2);
    }

    #[test]
    fn test_search() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Foo".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Bar".into(),
            "contains foo".into(),
            None,
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: Some("foo".into()),
                todo_status: None,
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

    // Tree-specific filtering tests

    #[test]
    fn test_active_filter_shows_nested_children() {
        let mut store = InMemoryStore::new();
        // Create parent
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        // Create child inside parent
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let res = run(&store, Scope::Project, PadFilter::default()).unwrap();

        // Should have 1 root pad (Parent)
        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Parent");
        // Parent should have 1 child
        assert_eq!(res.listed_pads[0].children.len(), 1);
        assert_eq!(res.listed_pads[0].children[0].pad.metadata.title, "Child");
    }

    #[test]
    fn test_active_filter_hides_deleted_child() {
        let mut store = InMemoryStore::new();
        // Create parent
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        // Create two children
        create::run(
            &mut store,
            Scope::Project,
            "Child1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child2".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Delete Child1 (newest child = 1.1)
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        )
        .unwrap();

        let res = run(&store, Scope::Project, PadFilter::default()).unwrap();

        // Parent should have only 1 visible child (Child1 deleted)
        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].children.len(), 1);
        assert_eq!(res.listed_pads[0].children[0].pad.metadata.title, "Child1");
    }

    #[test]
    fn test_deleted_filter_shows_parent_with_children() {
        let mut store = InMemoryStore::new();
        // Create parent with child
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Delete the parent (not the child)
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // View deleted pads
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Deleted,
                search_term: None,
                todo_status: None,
            },
        )
        .unwrap();

        // Should show deleted parent with its non-deleted child
        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Parent");
        // Child should be visible under deleted parent (per spec)
        assert_eq!(res.listed_pads[0].children.len(), 1);
        assert_eq!(res.listed_pads[0].children[0].pad.metadata.title, "Child");
    }

    #[test]
    fn test_active_filter_hides_children_of_deleted_parent() {
        let mut store = InMemoryStore::new();
        // Create parent with child
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Delete the parent
        delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // View active pads
        let res = run(&store, Scope::Project, PadFilter::default()).unwrap();

        // Should have no active roots (parent is deleted, child is hidden)
        assert_eq!(res.listed_pads.len(), 0);
    }

    // --- TodoStatus filtering tests ---

    #[test]
    fn test_todo_status_filter_planned() {
        let mut store = InMemoryStore::new();
        // Create pads with different statuses
        create::run(
            &mut store,
            Scope::Project,
            "Planned1".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Planned2".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Mark first pad as Done via direct store manipulation
        let pads = store.list_pads(Scope::Project).unwrap();
        let mut pad = pads
            .iter()
            .find(|p| p.metadata.title == "Planned1")
            .unwrap()
            .clone();
        pad.metadata.status = TodoStatus::Done;
        store.save_pad(&pad, Scope::Project).unwrap();

        // Filter for Planned only
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::Planned),
            },
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Planned2");
    }

    #[test]
    fn test_todo_status_filter_done() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Todo1".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Todo2".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Todo3".into(), "".into(), None).unwrap();

        // Mark first two as Done
        let pads = store.list_pads(Scope::Project).unwrap();
        for title in ["Todo1", "Todo2"] {
            let mut pad = pads
                .iter()
                .find(|p| p.metadata.title == title)
                .unwrap()
                .clone();
            pad.metadata.status = TodoStatus::Done;
            store.save_pad(&pad, Scope::Project).unwrap();
        }

        // Filter for Done only
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::Done),
            },
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);
        let titles: Vec<_> = res
            .listed_pads
            .iter()
            .map(|dp| dp.pad.metadata.title.as_str())
            .collect();
        assert!(titles.contains(&"Todo1"));
        assert!(titles.contains(&"Todo2"));
    }

    #[test]
    fn test_todo_status_filter_in_progress() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Task1".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Task2".into(), "".into(), None).unwrap();

        // Mark Task1 as InProgress
        let pads = store.list_pads(Scope::Project).unwrap();
        let mut pad = pads
            .iter()
            .find(|p| p.metadata.title == "Task1")
            .unwrap()
            .clone();
        pad.metadata.status = TodoStatus::InProgress;
        store.save_pad(&pad, Scope::Project).unwrap();

        // Filter for InProgress only
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::InProgress),
            },
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Task1");
    }

    #[test]
    fn test_todo_status_filter_none_shows_all() {
        let mut store = InMemoryStore::new();
        create::run(
            &mut store,
            Scope::Project,
            "Planned".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(&mut store, Scope::Project, "Done".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "InProgress".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Set statuses
        let pads = store.list_pads(Scope::Project).unwrap();

        let mut done_pad = pads
            .iter()
            .find(|p| p.metadata.title == "Done")
            .unwrap()
            .clone();
        done_pad.metadata.status = TodoStatus::Done;
        store.save_pad(&done_pad, Scope::Project).unwrap();

        let mut ip_pad = pads
            .iter()
            .find(|p| p.metadata.title == "InProgress")
            .unwrap()
            .clone();
        ip_pad.metadata.status = TodoStatus::InProgress;
        store.save_pad(&ip_pad, Scope::Project).unwrap();

        // Filter with None (show all)
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: None,
            },
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 3);
    }

    #[test]
    fn test_todo_status_filter_preserves_index() {
        // Per spec: "Statuses do not alter the canonical display index"
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "First".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Second".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Third".into(), "".into(), None).unwrap();

        // Pads are sorted newest first, so:
        // Third (newest) = index 1
        // Second = index 2
        // First (oldest) = index 3

        // Mark Second (index 2) as Done, others stay Planned
        let pads = store.list_pads(Scope::Project).unwrap();
        let mut pad = pads
            .iter()
            .find(|p| p.metadata.title == "Second")
            .unwrap()
            .clone();
        pad.metadata.status = TodoStatus::Done;
        store.save_pad(&pad, Scope::Project).unwrap();

        // Filter for Planned only
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::Planned),
            },
        )
        .unwrap();

        // Should show Third (1) and First (3), but Second (2) is filtered out
        // Indexes should remain 1 and 3, not renumbered to 1 and 2
        assert_eq!(res.listed_pads.len(), 2);

        // Third (newest) should still have index 1
        let third = res
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "Third")
            .unwrap();
        assert!(matches!(third.index, DisplayIndex::Regular(1)));

        // First (oldest) should still have index 3
        let first = res
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "First")
            .unwrap();
        assert!(matches!(first.index, DisplayIndex::Regular(3)));
    }

    #[test]
    fn test_todo_status_filter_with_nested_pads() {
        let mut store = InMemoryStore::new();

        // Create parent with children
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child2".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Mark Child1 as Done, Parent and Child2 stay Planned
        let pads = store.list_pads(Scope::Project).unwrap();
        let mut child1 = pads
            .iter()
            .find(|p| p.metadata.title == "Child1")
            .unwrap()
            .clone();
        child1.metadata.status = TodoStatus::Done;
        store.save_pad(&child1, Scope::Project).unwrap();

        // Filter for Planned only
        let res = run(
            &store,
            Scope::Project,
            PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::Planned),
            },
        )
        .unwrap();

        // Parent is Planned, so it shows
        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Parent");

        // Child2 is Planned, so it shows under parent
        // Child1 is Done, so it's filtered out
        assert_eq!(res.listed_pads[0].children.len(), 1);
        assert_eq!(res.listed_pads[0].children[0].pad.metadata.title, "Child2");
    }
}
