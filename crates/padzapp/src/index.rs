//! # Pad Identifiers: UUID vs Display Index
//!
//! Pads need to be referenced by ID. Since padz is a CLI tool, the primary interface is text.
//! While UUIDs are the correct technical choice for unique identification, they are cumbersome to type.
//!
//! Sequential IDs are the logical user-facing choice. However, naive sequential indexing
//! (numbering the current output list 1..N) creates ambiguity and "index drift".
//!
//! ## The Dual-Identifier Solution
//!
//! Padz uses a dual-identifier system:
//!
//! 1. **UUID (Internal)**: Immutable, canonical, globally unique.
//! 2. **Display Index (External)**: A stable integer generated from a canonical ordering.
//!
//! ## Canonical Ordering
//!
//! Even when filtering or searching, the ID assigned to a pad remains consistent with its
//! position in the full, unfiltered list. This ensures `padz delete 2` always targets the
//! same pad regardless of the current view.
//!
//! **Ordering Logic**:
//! - All pads sorted by `created_at` descending (Newest = 1)
//! - Pinned pads get an additional `p1`, `p2`... index (appear in both pinned and regular lists)
//! - Deleted pads: Separate bucket `d1`, `d2`...
//!
//! ## Pinned Pads Have Two Indexes
//!
//! A pinned pad appears **twice** in the indexed list:
//! - Once with a `Pinned` index (`p1`, `p2`, etc.)
//! - Once with its canonical `Regular` index (`1`, `2`, etc.)
//!
//! This ensures stability when a pad is unpinned—the regular index remains the same.
//!
//! ## Implementation
//!
//! - [`index_pads`]: Assigns canonical display indexes to a list of pads
//! - [`DisplayIndex`]: The user-facing index enum (`Regular`, `Pinned`, `Deleted`)
//! - [`DisplayPad`]: Connects a `Pad` with its `DisplayIndex`
//! - [`parse_index_or_range`]: Parses user input like `"1-3"` into `Vec<DisplayIndex>`
//!
//! **Developer Note**: When implementing list/view commands, always use [`index_pads`].
//! Never manually enumerate a list of pads, as you will break the canonical ID association.
//!
//! For input resolution (mapping indexes to UUIDs), see the [`crate::api`] module.

use crate::model::Pad;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

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
    Path(Vec<DisplayIndex>),
    Range(Vec<DisplayIndex>, Vec<DisplayIndex>), // Start Path, End Path
    Title(String),
}

impl std::fmt::Display for PadSelector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PadSelector::Path(path) => {
                let s: Vec<String> = path.iter().map(|idx| idx.to_string()).collect();
                write!(f, "{}", s.join("."))
            }
            PadSelector::Range(start, end) => {
                let s_start: Vec<String> = start.iter().map(|idx| idx.to_string()).collect();
                let s_end: Vec<String> = end.iter().map(|idx| idx.to_string()).collect();
                write!(f, "{}-{}", s_start.join("."), s_end.join("."))
            }
            PadSelector::Title(t) => write!(f, "\"{}\"", t),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DisplayPad {
    pub pad: Pad,
    pub index: DisplayIndex,
    pub matches: Option<Vec<SearchMatch>>,
    pub children: Vec<DisplayPad>,
}

/// Assigns canonical display indexes to a list of pads, building a tree structure.
///
/// **Per-parent bucketing**: The same pinned/regular/deleted indexing logic is applied
/// recursively at each nesting level. Each parent maintains its own index namespace:
/// - Root level: `p1`, `1`, `2`, `d1`
/// - Children of pad 1: `1.p1`, `1.1`, `1.2`, `1.d1`
/// - Children of pad 1.2: `1.2.p1`, `1.2.1`, etc.
///
/// **Dual indexing**: Pinned pads appear **twice** at each level—once with a `Pinned`
/// index and once with a `Regular` index. This ensures stability when unpinning.
///
/// The returned list is ordered: pinned entries first, then regular, then deleted.
/// Each entry's `children` vector follows the same ordering recursively.
pub fn index_pads(pads: Vec<Pad>) -> Vec<DisplayPad> {
    // Group pads by parent_id
    let mut parent_map: HashMap<Option<Uuid>, Vec<Pad>> = HashMap::new();
    for pad in pads {
        parent_map
            .entry(pad.metadata.parent_id)
            .or_default()
            .push(pad);
    }

    // Process roots (parent_id = None), recursively indexing their children
    let root_pads = parent_map.remove(&None).unwrap_or_default();
    index_level(root_pads, &parent_map)
}

/// Indexes a single level of the tree (siblings with the same parent).
///
/// Applies the standard three-pass indexing at this level:
/// 1. **Pinned pass**: Assigns `Pinned(1)`, `Pinned(2)`, etc. to pinned non-deleted pads
/// 2. **Regular pass**: Assigns `Regular(1)`, `Regular(2)`, etc. to ALL non-deleted pads
/// 3. **Deleted pass**: Assigns `Deleted(1)`, `Deleted(2)`, etc. to deleted pads
///
/// Note: Pinned pads get entries in BOTH the pinned and regular passes (dual indexing).
/// This is recursive—each pad's children are indexed the same way.
fn index_level(
    mut pads: Vec<Pad>,
    parent_map: &HashMap<Option<Uuid>, Vec<Pad>>,
) -> Vec<DisplayPad> {
    // Sort by created_at descending (newest first) within this level
    pads.sort_by(|a, b| b.metadata.created_at.cmp(&a.metadata.created_at));

    let mut results = Vec::new();

    // Helper closure to build DisplayPad and recurse
    let mut add_pad = |pad: Pad, index: DisplayIndex| {
        let children = parent_map
            .get(&Some(pad.metadata.id))
            .cloned()
            .unwrap_or_default();

        // Recurse for children
        let indexed_children = index_level(children, parent_map);

        results.push(DisplayPad {
            pad,
            index,
            matches: None,
            children: indexed_children,
        });
    };

    // First pass: Pinned
    let mut pinned_idx = 1;
    for pad in &pads {
        if pad.metadata.is_pinned && !pad.metadata.is_deleted {
            add_pad(pad.clone(), DisplayIndex::Pinned(pinned_idx));
            pinned_idx += 1;
        }
    }

    // Second pass: Regular (all non-deleted)
    let mut regular_idx = 1;
    for pad in &pads {
        if !pad.metadata.is_deleted {
            add_pad(pad.clone(), DisplayIndex::Regular(regular_idx));
            regular_idx += 1;
        }
    }

    // Third pass: Deleted
    let mut deleted_idx = 1;
    for pad in &pads {
        if pad.metadata.is_deleted {
            add_pad(pad.clone(), DisplayIndex::Deleted(deleted_idx));
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

/// Parses a single input string that may be either a path or a range of paths.
///
/// Supports formats:
/// - Path: "3", "3.1", "p1", "d2.1"
/// - Range: "1-3", "1.1-1.3", "1.2-2.1"
pub fn parse_index_or_range(s: &str) -> Result<PadSelector, String> {
    // Check if it's a range (contains '-' but not at the start for negative numbers)
    // We need to be careful: "p1-p3" has '-' in the middle
    if let Some(dash_pos) = s.find('-') {
        // Don't treat leading '-' as a range separator
        if dash_pos > 0 {
            let start_str = &s[..dash_pos];
            let end_str = &s[dash_pos + 1..];

            // Parse endpoints as paths
            let start_path = parse_path(start_str)?;
            let end_path = parse_path(end_str)?;

            return Ok(PadSelector::Range(start_path, end_path));
        }
    }

    // Not a range
    parse_path(s).map(PadSelector::Path)
}

/// Parses a dot-separated path string into a vector of DisplayIndex.
/// e.g. "1.2" -> [Regular(1), Regular(2)]
/// "p1" -> [Pinned(1)]
fn parse_path(s: &str) -> Result<Vec<DisplayIndex>, String> {
    s.split('.').map(DisplayIndex::from_str).collect()
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
            Ok(PadSelector::Path(vec![DisplayIndex::Regular(3)]))
        );
        assert_eq!(
            parse_index_or_range("p2"),
            Ok(PadSelector::Path(vec![DisplayIndex::Pinned(2)]))
        );
        assert_eq!(
            parse_index_or_range("d1"),
            Ok(PadSelector::Path(vec![DisplayIndex::Deleted(1)]))
        );
    }

    #[test]
    fn test_parse_regular_range() {
        assert_eq!(
            parse_index_or_range("3-5"),
            Ok(PadSelector::Range(
                vec![DisplayIndex::Regular(3)],
                vec![DisplayIndex::Regular(5)]
            ))
        );

        // Single element range (start == end)
        assert_eq!(
            parse_index_or_range("3-3"),
            Ok(PadSelector::Range(
                vec![DisplayIndex::Regular(3)],
                vec![DisplayIndex::Regular(3)]
            ))
        );
    }

    #[test]
    fn test_parse_pinned_range() {
        assert_eq!(
            parse_index_or_range("p1-p3"),
            Ok(PadSelector::Range(
                vec![DisplayIndex::Pinned(1)],
                vec![DisplayIndex::Pinned(3)]
            ))
        );
    }

    #[test]
    fn test_parse_deleted_range() {
        assert_eq!(
            parse_index_or_range("d2-d4"),
            Ok(PadSelector::Range(
                vec![DisplayIndex::Deleted(2)],
                vec![DisplayIndex::Deleted(4)]
            ))
        );
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
