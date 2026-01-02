use crate::model::Pad;
use std::str::FromStr;

/// A segment of text in a search match, either plain text or a matched term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchSegment {
    Plain(String),
    Match(String),
}

/// A line containing a search match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub line_number: usize, // 0 for title, 1+ for content lines
    pub segments: Vec<MatchSegment>,
}

/// A user-facing index for a pad.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum DisplayIndex {
    Pinned(usize),
    Regular(usize),
    Deleted(usize),
}

impl std::fmt::Display for DisplayIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayIndex::Pinned(i) => write!(f, "p{}", i),
            DisplayIndex::Regular(i) => write!(f, "{}", i),
            DisplayIndex::Deleted(i) => write!(f, "d{}", i),
        }
    }
}

/// A user input to select a pad, either by its index or a search term for its title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PadSelector {
    Index(DisplayIndex),
    Title(String),
}

impl std::fmt::Display for PadSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PadSelector::Index(idx) => write!(f, "{}", idx),
            PadSelector::Title(t) => write!(f, "\"{}\"", t),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DisplayPad {
    pub pad: Pad,
    pub index: DisplayIndex,
    pub matches: Option<Vec<SearchMatch>>,
}

/// Assigns canonical display indexes to a list of pads.
///
/// Returns a flat list of [`DisplayPad`] entries. Note that pinned pads will
/// appear **twice**: once with a `Pinned` index and once with a `Regular` index.
/// This is intentional—see module documentation for rationale.
///
/// The returned list is ordered: pinned entries first, then regular, then deleted.
pub fn index_pads(mut pads: Vec<Pad>) -> Vec<DisplayPad> {
    // Sort by created_at descending (newest first) for stable ordering
    pads.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));

    let mut results = Vec::new();

    // First pass: assign pinned indexes (p1, p2, ...)
    let mut pinned_idx = 1;
    for pad in &pads {
        if pad.metadata.is_pinned && !pad.metadata.is_deleted {
            results.push(DisplayPad {
                pad: pad.clone(),
                index: DisplayIndex::Pinned(pinned_idx),
                matches: None,
            });
            pinned_idx += 1;
        }
    }

    // Second pass: assign regular indexes (1, 2, ...) to ALL non-deleted pads
    // including pinned ones - this ensures canonical indexes are stable
    let mut regular_idx = 1;
    for pad in &pads {
        if !pad.metadata.is_deleted {
            results.push(DisplayPad {
                pad: pad.clone(),
                index: DisplayIndex::Regular(regular_idx),
                matches: None,
            });
            regular_idx += 1;
        }
    }

    // Third pass: assign deleted indexes (d1, d2, ...)
    let mut deleted_idx = 1;
    for pad in &pads {
        if pad.metadata.is_deleted {
            results.push(DisplayPad {
                pad: pad.clone(),
                index: DisplayIndex::Deleted(deleted_idx),
                matches: None,
            });
            deleted_idx += 1;
        }
    }

    results
}

impl std::str::FromStr for DisplayIndex {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(rest) = s.strip_prefix('p') {
            if let Ok(n) = rest.parse() {
                return Ok(DisplayIndex::Pinned(n));
            }
        }
        if let Some(rest) = s.strip_prefix('d') {
            if let Ok(n) = rest.parse() {
                return Ok(DisplayIndex::Deleted(n));
            }
        }
        if let Ok(n) = s.parse() {
            return Ok(DisplayIndex::Regular(n));
        }
        Err(format!("Invalid index format: {}", s))
    }
}

/// Parses a single input string that may be either a single index or a range.
///
/// Supports formats:
/// - Single index: "3", "p1", "d2"
/// - Range: "3-5" (expands to 3, 4, 5), "p1-p3" (expands to p1, p2, p3)
///
/// Range rules:
/// - Both endpoints must be the same type (Regular, Pinned, or Deleted)
/// - Start must be <= end (e.g., "3-3" is valid, "3-2" is an error)
/// - Validation that the indexes actually exist happens later during resolution
pub fn parse_index_or_range(s: &str) -> Result<Vec<DisplayIndex>, String> {
    // Check if it's a range (contains '-' but not at the start for negative numbers)
    // We need to be careful: "p1-p3" has '-' in the middle
    if let Some(dash_pos) = s.find('-') {
        // Don't treat leading '-' as a range separator (though we don't support negative indexes)
        if dash_pos > 0 {
            let start_str = &s[..dash_pos];
            let end_str = &s[dash_pos + 1..];

            // Try to parse both as DisplayIndex
            let start = DisplayIndex::from_str(start_str)?;
            let end = DisplayIndex::from_str(end_str)?;

            return expand_range(start, end);
        }
    }

    // Not a range, parse as single index
    DisplayIndex::from_str(s).map(|idx| vec![idx])
}

/// Expands a range of DisplayIndex values.
///
/// Both endpoints must be the same variant (Regular, Pinned, or Deleted).
/// Start must be <= end.
fn expand_range(start: DisplayIndex, end: DisplayIndex) -> Result<Vec<DisplayIndex>, String> {
    match (&start, &end) {
        (DisplayIndex::Regular(s), DisplayIndex::Regular(e)) => {
            if s > e {
                return Err(format!(
                    "Invalid range: start ({}) must be <= end ({})",
                    s, e
                ));
            }
            Ok((*s..=*e).map(DisplayIndex::Regular).collect())
        }
        (DisplayIndex::Pinned(s), DisplayIndex::Pinned(e)) => {
            if s > e {
                return Err(format!(
                    "Invalid range: start (p{}) must be <= end (p{})",
                    s, e
                ));
            }
            Ok((*s..=*e).map(DisplayIndex::Pinned).collect())
        }
        (DisplayIndex::Deleted(s), DisplayIndex::Deleted(e)) => {
            if s > e {
                return Err(format!(
                    "Invalid range: start (d{}) must be <= end (d{})",
                    s, e
                ));
            }
            Ok((*s..=*e).map(DisplayIndex::Deleted).collect())
        }
        _ => Err(format!(
            "Invalid range: cannot mix index types ({} and {})",
            start, end
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_pad(title: &str, pinned: bool, deleted: bool) -> Pad {
        let mut p = Pad::new(title.to_string(), "".to_string());
        p.metadata.is_pinned = pinned;
        p.metadata.is_deleted = deleted;
        p
    }

    #[test]
    fn test_indexing_buckets() {
        let p1 = make_pad("Regular 1", false, false);
        let p2 = make_pad("Pinned 1", true, false);
        let p3 = make_pad("Deleted 1", false, true);
        let p4 = make_pad("Regular 2", false, false);

        let pads = vec![p1, p2, p3, p4];
        let indexed = index_pads(pads);

        // With the canonical indexing (newest first), pinned pads appear in BOTH
        // the pinned list AND the regular list.
        // Creation order: Regular 1, Pinned 1, Deleted 1, Regular 2
        // Reverse chronological: Regular 2, Deleted 1, Pinned 1, Regular 1
        // Expected entries:
        // - p1: Pinned 1 (only pinned non-deleted pad)
        // - 1: Regular 2 (newest non-deleted)
        // - 2: Pinned 1 (second newest non-deleted)
        // - 3: Regular 1 (oldest non-deleted)
        // - d1: Deleted 1

        // Check pinned index
        let pinned_entries: Vec<_> = indexed
            .iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Pinned(_)))
            .collect();
        assert_eq!(pinned_entries.len(), 1);
        assert_eq!(pinned_entries[0].pad.metadata.title, "Pinned 1");
        assert_eq!(pinned_entries[0].index, DisplayIndex::Pinned(1));

        // Check regular indexes - should include ALL non-deleted pads (newest first)
        let regular_entries: Vec<_> = indexed
            .iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Regular(_)))
            .collect();
        assert_eq!(regular_entries.len(), 3);
        assert_eq!(regular_entries[0].pad.metadata.title, "Regular 2"); // newest = 1
        assert_eq!(regular_entries[0].index, DisplayIndex::Regular(1));
        assert_eq!(regular_entries[2].pad.metadata.title, "Regular 1"); // oldest = 3
        assert_eq!(regular_entries[2].index, DisplayIndex::Regular(3));

        // Check deleted index
        let deleted_entries: Vec<_> = indexed
            .iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Deleted(_)))
            .collect();
        assert_eq!(deleted_entries.len(), 1);
        assert_eq!(deleted_entries[0].pad.metadata.title, "Deleted 1");
    }

    #[test]
    fn test_pinned_pad_has_both_indexes() {
        let p1 = make_pad("Note A", false, false);
        let p2 = make_pad("Note B", true, false); // pinned
        let p3 = make_pad("Note C", false, false);

        let pads = vec![p1, p2, p3];
        let indexed = index_pads(pads);

        // Creation order: Note A, Note B, Note C
        // Reverse chronological: Note C (1), Note B (2), Note A (3)
        // Note B should appear twice: as p1 and as regular index 2
        let note_b_entries: Vec<_> = indexed
            .iter()
            .filter(|dp| dp.pad.metadata.title == "Note B")
            .collect();
        assert_eq!(note_b_entries.len(), 2);

        // One should be Pinned(1)
        assert!(note_b_entries
            .iter()
            .any(|dp| dp.index == DisplayIndex::Pinned(1)));
        // One should be Regular(2) - it's the second newest pad
        assert!(note_b_entries
            .iter()
            .any(|dp| dp.index == DisplayIndex::Regular(2)));
    }

    #[test]
    fn test_parsing() {
        use std::str::FromStr;

        assert_eq!(DisplayIndex::from_str("1"), Ok(DisplayIndex::Regular(1)));
        assert_eq!(DisplayIndex::from_str("42"), Ok(DisplayIndex::Regular(42)));
        assert_eq!(DisplayIndex::from_str("p1"), Ok(DisplayIndex::Pinned(1)));
        assert_eq!(DisplayIndex::from_str("p99"), Ok(DisplayIndex::Pinned(99)));
        assert_eq!(DisplayIndex::from_str("d1"), Ok(DisplayIndex::Deleted(1)));
        assert_eq!(DisplayIndex::from_str("d5"), Ok(DisplayIndex::Deleted(5)));

        assert!(DisplayIndex::from_str("").is_err());
        assert!(DisplayIndex::from_str("abc").is_err());
        assert!(DisplayIndex::from_str("p").is_err());
        assert!(DisplayIndex::from_str("d").is_err());
        assert!(DisplayIndex::from_str("12a").is_err());
        assert!(DisplayIndex::from_str("p1a").is_err());
    }

    #[test]
    fn test_parse_single_index() {
        // Single indexes should return a vec with one element
        assert_eq!(
            parse_index_or_range("3"),
            Ok(vec![DisplayIndex::Regular(3)])
        );
        assert_eq!(
            parse_index_or_range("p2"),
            Ok(vec![DisplayIndex::Pinned(2)])
        );
        assert_eq!(
            parse_index_or_range("d1"),
            Ok(vec![DisplayIndex::Deleted(1)])
        );
    }

    #[test]
    fn test_parse_regular_range() {
        assert_eq!(
            parse_index_or_range("3-5"),
            Ok(vec![
                DisplayIndex::Regular(3),
                DisplayIndex::Regular(4),
                DisplayIndex::Regular(5)
            ])
        );

        // Single element range (start == end)
        assert_eq!(
            parse_index_or_range("3-3"),
            Ok(vec![DisplayIndex::Regular(3)])
        );
    }

    #[test]
    fn test_parse_pinned_range() {
        assert_eq!(
            parse_index_or_range("p1-p3"),
            Ok(vec![
                DisplayIndex::Pinned(1),
                DisplayIndex::Pinned(2),
                DisplayIndex::Pinned(3)
            ])
        );
    }

    #[test]
    fn test_parse_deleted_range() {
        assert_eq!(
            parse_index_or_range("d2-d4"),
            Ok(vec![
                DisplayIndex::Deleted(2),
                DisplayIndex::Deleted(3),
                DisplayIndex::Deleted(4)
            ])
        );
    }

    #[test]
    fn test_parse_range_invalid_order() {
        // Start > end should error
        let result = parse_index_or_range("5-3");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be <= end"));

        let result = parse_index_or_range("p3-p1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be <= end"));
    }

    #[test]
    fn test_parse_range_mixed_types() {
        // Cannot mix Regular and Pinned
        let result = parse_index_or_range("1-p3");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot mix index types"));

        // Cannot mix Pinned and Deleted
        let result = parse_index_or_range("p1-d3");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("cannot mix index types"));
    }

    #[test]
    fn test_parse_range_invalid_format() {
        // Invalid start
        let result = parse_index_or_range("abc-5");
        assert!(result.is_err());

        // Invalid end
        let result = parse_index_or_range("3-xyz");
        assert!(result.is_err());

        // Empty parts
        let result = parse_index_or_range("-5");
        assert!(result.is_err());

        let result = parse_index_or_range("3-");
        assert!(result.is_err());
    }
}
