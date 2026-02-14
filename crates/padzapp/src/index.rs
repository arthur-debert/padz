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
use serde::Serialize;
use std::collections::HashMap;
use std::str::FromStr;
use uuid::Uuid;

/// A segment of text in a search match, either plain text or a matched term.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", content = "text")]
pub enum MatchSegment {
    Plain(String),
    Match(String),
}

/// A line containing a search match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchMatch {
    pub line_number: usize, // 0 for title, 1+ for content lines
    pub segments: Vec<MatchSegment>,
}

/// A user-facing index for a pad.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(tag = "type", content = "value")]
pub enum DisplayIndex {
    Pinned(usize),
    Regular(usize),
    Archived(usize),
    Deleted(usize),
}

impl std::fmt::Display for DisplayIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayIndex::Pinned(i) => write!(f, "p{}", i),
            DisplayIndex::Regular(i) => write!(f, "{}", i),
            DisplayIndex::Archived(i) => write!(f, "ar{}", i),
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

#[derive(Debug, Clone, Serialize)]
pub struct DisplayPad {
    pub pad: Pad,
    pub index: DisplayIndex,
    pub matches: Option<Vec<SearchMatch>>,
    pub children: Vec<DisplayPad>,
}

/// Internal tag for which bucket a pad belongs to during indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexBucket {
    Active,
    Archived,
    Deleted,
}

/// A pad tagged with its bucket membership for indexing purposes.
#[derive(Debug, Clone)]
struct TaggedPad {
    pad: Pad,
    bucket: IndexBucket,
}

/// Assigns canonical display indexes to pads from three lifecycle buckets,
/// building a tree structure.
///
/// **Per-parent bucketing**: The same pinned/regular/archived/deleted indexing logic
/// is applied recursively at each nesting level. Each parent maintains its own index namespace:
/// - Root level: `p1`, `1`, `2`, `ar1`, `d1`
/// - Children of pad 1: `1.p1`, `1.1`, `1.2`, `1.ar1`, `1.d1`
///
/// **Dual indexing**: Pinned active pads appear **twice** at each level—once with a `Pinned`
/// index and once with a `Regular` index. This ensures stability when unpinning.
///
/// The returned list is ordered: pinned, regular, archived, deleted.
/// Each entry's `children` vector follows the same ordering recursively.
pub fn index_pads(active: Vec<Pad>, archived: Vec<Pad>, deleted: Vec<Pad>) -> Vec<DisplayPad> {
    // Tag each pad with its bucket
    let mut all_tagged: Vec<TaggedPad> = Vec::new();
    for pad in active {
        all_tagged.push(TaggedPad {
            pad,
            bucket: IndexBucket::Active,
        });
    }
    for pad in archived {
        all_tagged.push(TaggedPad {
            pad,
            bucket: IndexBucket::Archived,
        });
    }
    for pad in deleted {
        all_tagged.push(TaggedPad {
            pad,
            bucket: IndexBucket::Deleted,
        });
    }

    // Group by parent_id
    let mut parent_map: HashMap<Option<Uuid>, Vec<TaggedPad>> = HashMap::new();
    for tagged in all_tagged {
        parent_map
            .entry(tagged.pad.metadata.parent_id)
            .or_default()
            .push(tagged);
    }

    // Process roots (parent_id = None), recursively indexing their children
    let root_pads = parent_map.remove(&None).unwrap_or_default();
    index_level(root_pads, &parent_map)
}

/// Indexes a single level of the tree (siblings with the same parent).
///
/// Applies four-pass indexing at this level:
/// 1. **Pinned pass**: `Pinned(1)`, `Pinned(2)`, etc. for pinned active pads
/// 2. **Regular pass**: `Regular(1)`, `Regular(2)`, etc. for ALL active pads
/// 3. **Archived pass**: `Archived(1)`, `Archived(2)`, etc. for archived pads
/// 4. **Deleted pass**: `Deleted(1)`, `Deleted(2)`, etc. for deleted pads
///
/// Note: Pinned active pads get entries in BOTH the pinned and regular passes (dual indexing).
/// This is recursive—each pad's children are indexed the same way.
fn index_level(
    mut pads: Vec<TaggedPad>,
    parent_map: &HashMap<Option<Uuid>, Vec<TaggedPad>>,
) -> Vec<DisplayPad> {
    // Sort by created_at descending (newest first) within this level
    pads.sort_by(|a, b| b.pad.metadata.created_at.cmp(&a.pad.metadata.created_at));

    let mut results = Vec::new();

    // Helper closure to build DisplayPad and recurse
    let mut add_pad = |tagged: TaggedPad, index: DisplayIndex| {
        let children = parent_map
            .get(&Some(tagged.pad.metadata.id))
            .cloned()
            .unwrap_or_default();

        // Recurse for children
        let indexed_children = index_level(children, parent_map);

        results.push(DisplayPad {
            pad: tagged.pad,
            index,
            matches: None,
            children: indexed_children,
        });
    };

    // First pass: Pinned (active + pinned)
    let mut pinned_idx = 1;
    for tagged in &pads {
        if tagged.bucket == IndexBucket::Active && tagged.pad.metadata.is_pinned {
            add_pad(tagged.clone(), DisplayIndex::Pinned(pinned_idx));
            pinned_idx += 1;
        }
    }

    // Second pass: Regular (all active)
    let mut regular_idx = 1;
    for tagged in &pads {
        if tagged.bucket == IndexBucket::Active {
            add_pad(tagged.clone(), DisplayIndex::Regular(regular_idx));
            regular_idx += 1;
        }
    }

    // Third pass: Archived
    let mut archived_idx = 1;
    for tagged in &pads {
        if tagged.bucket == IndexBucket::Archived {
            add_pad(tagged.clone(), DisplayIndex::Archived(archived_idx));
            archived_idx += 1;
        }
    }

    // Fourth pass: Deleted
    let mut deleted_idx = 1;
    for tagged in &pads {
        if tagged.bucket == IndexBucket::Deleted {
            add_pad(tagged.clone(), DisplayIndex::Deleted(deleted_idx));
            deleted_idx += 1;
        }
    }

    results
}

impl std::str::FromStr for DisplayIndex {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Check "ar" before single-char prefixes to avoid ambiguity
        if let Some(rest) = s.strip_prefix("ar") {
            if let Ok(n) = rest.parse() {
                return Ok(DisplayIndex::Archived(n));
            }
        }
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

    fn make_pad(title: &str, pinned: bool) -> Pad {
        let mut p = Pad::new(title.to_string(), "".to_string());
        p.metadata.is_pinned = pinned;
        p
    }

    #[test]
    fn test_indexing_buckets() {
        let p1 = make_pad("Regular 1", false);
        let p2 = make_pad("Pinned 1", true);
        let p3 = make_pad("Deleted 1", false);
        let p4 = make_pad("Regular 2", false);

        let active = vec![p1, p2, p4];
        let deleted = vec![p3];
        let indexed = index_pads(active, vec![], deleted);

        // With the canonical indexing (newest first), pinned pads appear in BOTH
        // the pinned list AND the regular list.
        // Active creation order: Regular 1, Pinned 1, Regular 2
        // Active reverse chronological: Regular 2, Pinned 1, Regular 1
        // Expected entries:
        // - p1: Pinned 1 (only pinned active pad)
        // - 1: Regular 2 (newest active)
        // - 2: Pinned 1 (second newest active)
        // - 3: Regular 1 (oldest active)
        // - d1: Deleted 1

        // Check pinned index
        let pinned_entries: Vec<_> = indexed
            .iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Pinned(_)))
            .collect();
        assert_eq!(pinned_entries.len(), 1);
        assert_eq!(pinned_entries[0].pad.metadata.title, "Pinned 1");
        assert_eq!(pinned_entries[0].index, DisplayIndex::Pinned(1));

        // Check regular indexes - should include ALL active pads (newest first)
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
        let p1 = make_pad("Note A", false);
        let p2 = make_pad("Note B", true); // pinned
        let p3 = make_pad("Note C", false);

        let indexed = index_pads(vec![p1, p2, p3], vec![], vec![]);

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
        assert_eq!(DisplayIndex::from_str("ar1"), Ok(DisplayIndex::Archived(1)));
        assert_eq!(
            DisplayIndex::from_str("ar99"),
            Ok(DisplayIndex::Archived(99))
        );

        assert!(DisplayIndex::from_str("").is_err());
        assert!(DisplayIndex::from_str("abc").is_err());
        assert!(DisplayIndex::from_str("p").is_err());
        assert!(DisplayIndex::from_str("d").is_err());
        assert!(DisplayIndex::from_str("ar").is_err());
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
        assert_eq!(
            parse_index_or_range("ar3"),
            Ok(PadSelector::Path(vec![DisplayIndex::Archived(3)]))
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
    fn test_parse_archived_range() {
        assert_eq!(
            parse_index_or_range("ar1-ar5"),
            Ok(PadSelector::Range(
                vec![DisplayIndex::Archived(1)],
                vec![DisplayIndex::Archived(5)]
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

    // ==================== Tree-specific tests ====================

    #[test]
    fn test_parse_nested_path() {
        // Path notation: 1.2.3 means child 3 of child 2 of root 1
        assert_eq!(
            parse_index_or_range("1.2"),
            Ok(PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(2)
            ]))
        );
        assert_eq!(
            parse_index_or_range("1.2.3"),
            Ok(PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(2),
                DisplayIndex::Regular(3)
            ]))
        );
    }

    #[test]
    fn test_parse_nested_pinned_path() {
        // Pinned child of root 1: 1.p1
        assert_eq!(
            parse_index_or_range("1.p1"),
            Ok(PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Pinned(1)
            ]))
        );
        // Deeply nested pinned: 1.2.p1
        assert_eq!(
            parse_index_or_range("1.2.p1"),
            Ok(PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(2),
                DisplayIndex::Pinned(1)
            ]))
        );
    }

    #[test]
    fn test_parse_nested_range() {
        // Range within a tree: 1.1-1.3
        assert_eq!(
            parse_index_or_range("1.1-1.3"),
            Ok(PadSelector::Range(
                vec![DisplayIndex::Regular(1), DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(1), DisplayIndex::Regular(3)]
            ))
        );
        // Cross-parent range: 1.2-2.1
        assert_eq!(
            parse_index_or_range("1.2-2.1"),
            Ok(PadSelector::Range(
                vec![DisplayIndex::Regular(1), DisplayIndex::Regular(2)],
                vec![DisplayIndex::Regular(2), DisplayIndex::Regular(1)]
            ))
        );
    }

    #[test]
    fn test_tree_with_nested_children() {
        // Build a tree: Root -> Child -> Grandchild (all active)
        let mut grandchild = make_pad("Grandchild", false);
        let mut child = make_pad("Child", false);
        let root = make_pad("Root", false);

        // Set up parent relationships
        child.metadata.parent_id = Some(root.metadata.id);
        grandchild.metadata.parent_id = Some(child.metadata.id);

        let indexed = index_pads(vec![root, child, grandchild], vec![], vec![]);

        // Should have 1 root
        assert_eq!(indexed.len(), 1);
        assert_eq!(indexed[0].pad.metadata.title, "Root");
        assert_eq!(indexed[0].index, DisplayIndex::Regular(1));

        // Root should have 1 child
        assert_eq!(indexed[0].children.len(), 1);
        assert_eq!(indexed[0].children[0].pad.metadata.title, "Child");
        assert_eq!(indexed[0].children[0].index, DisplayIndex::Regular(1));

        // Child should have 1 grandchild
        assert_eq!(indexed[0].children[0].children.len(), 1);
        assert_eq!(
            indexed[0].children[0].children[0].pad.metadata.title,
            "Grandchild"
        );
        assert_eq!(
            indexed[0].children[0].children[0].index,
            DisplayIndex::Regular(1)
        );
    }

    #[test]
    fn test_tree_pinned_child_has_dual_index() {
        // Root with a pinned child - child should appear twice in children
        let mut child = make_pad("Pinned Child", true);
        let root = make_pad("Root", false);

        child.metadata.parent_id = Some(root.metadata.id);

        let indexed = index_pads(vec![root, child], vec![], vec![]);

        // Root's children should have 2 entries for the pinned child
        assert_eq!(indexed[0].children.len(), 2);

        // One as Pinned(1)
        let pinned_child = indexed[0]
            .children
            .iter()
            .find(|c| matches!(c.index, DisplayIndex::Pinned(_)));
        assert!(pinned_child.is_some());
        assert_eq!(pinned_child.unwrap().index, DisplayIndex::Pinned(1));

        // One as Regular(1)
        let regular_child = indexed[0]
            .children
            .iter()
            .find(|c| matches!(c.index, DisplayIndex::Regular(_)));
        assert!(regular_child.is_some());
        assert_eq!(regular_child.unwrap().index, DisplayIndex::Regular(1));
    }

    #[test]
    fn test_tree_deep_nesting_four_levels() {
        // Create 4-level deep tree: L1 -> L2 -> L3 -> L4 (all active)
        let mut l4 = make_pad("Level 4", false);
        let mut l3 = make_pad("Level 3", false);
        let mut l2 = make_pad("Level 2", false);
        let l1 = make_pad("Level 1", false);

        l2.metadata.parent_id = Some(l1.metadata.id);
        l3.metadata.parent_id = Some(l2.metadata.id);
        l4.metadata.parent_id = Some(l3.metadata.id);

        let indexed = index_pads(vec![l1, l2, l3, l4], vec![], vec![]);

        // Navigate to L4: indexed[0].children[0].children[0].children[0]
        assert_eq!(indexed[0].pad.metadata.title, "Level 1");
        assert_eq!(indexed[0].children[0].pad.metadata.title, "Level 2");
        assert_eq!(
            indexed[0].children[0].children[0].pad.metadata.title,
            "Level 3"
        );
        assert_eq!(
            indexed[0].children[0].children[0].children[0]
                .pad
                .metadata
                .title,
            "Level 4"
        );

        // Each level should have index Regular(1) within its parent
        assert_eq!(indexed[0].index, DisplayIndex::Regular(1));
        assert_eq!(indexed[0].children[0].index, DisplayIndex::Regular(1));
        assert_eq!(
            indexed[0].children[0].children[0].index,
            DisplayIndex::Regular(1)
        );
        assert_eq!(
            indexed[0].children[0].children[0].children[0].index,
            DisplayIndex::Regular(1)
        );
    }

    #[test]
    fn test_archived_pads_get_archived_index() {
        let p1 = make_pad("Active 1", false);
        let p2 = make_pad("Archived 1", false);
        let p3 = make_pad("Archived 2", false);

        let indexed = index_pads(vec![p1], vec![p2, p3], vec![]);

        let archived_entries: Vec<_> = indexed
            .iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Archived(_)))
            .collect();
        assert_eq!(archived_entries.len(), 2);

        let regular_entries: Vec<_> = indexed
            .iter()
            .filter(|dp| matches!(dp.index, DisplayIndex::Regular(_)))
            .collect();
        assert_eq!(regular_entries.len(), 1);
    }

    #[test]
    fn test_all_three_buckets() {
        let active = make_pad("Active", false);
        let archived = make_pad("Archived", false);
        let deleted = make_pad("Deleted", false);

        let indexed = index_pads(vec![active], vec![archived], vec![deleted]);

        assert_eq!(indexed.len(), 3);

        assert!(indexed
            .iter()
            .any(|dp| matches!(dp.index, DisplayIndex::Regular(_))));
        assert!(indexed
            .iter()
            .any(|dp| matches!(dp.index, DisplayIndex::Archived(_))));
        assert!(indexed
            .iter()
            .any(|dp| matches!(dp.index, DisplayIndex::Deleted(_))));
    }
}
