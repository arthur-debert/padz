//! # Display Indexing System
//!
//! In a high-frequency shell application where users constantly refer to items by ID,
//! auto-generated identifiers like UUIDs—or even large integers—become unwieldy to type
//! repeatedly (e.g., `delete 34343 342334` instead of `delete 3 8`).
//!
//! ## Dual ID Design
//!
//! Padz uses a dual ID system:
//! - **UUID**: Unambiguously identifies each pad (used internally, in storage)
//! - **Display Index**: Ergonomic integer, small, and self-compacting (used in CLI)
//!
//! Display indexes don't grow unboundedly—they compact as items are deleted.
//!
//! ## Canonical Ordering
//!
//! The key insight is defining a **canonical view**: the "normal" way to see data.
//! Display indexes are simply positions in this canonical ordering.
//!
//! For padz, the canonical view is:
//! - Non-deleted pads
//! - Reverse chronological order (newest first)
//! - No filtering applied
//!
//! Items outside this "bucket" (deleted, pinned) have their own index namespaces.
//!
//! ## Why This Matters
//!
//! This design makes indexes **stable across filtered views**. You can:
//!
//! ```text
//! $ padz search "meeting"     # Shows pads 2 and 4 match
//! $ padz delete 2             # Deletes pad 2—unambiguously
//! ```
//!
//! Because search results use canonical indexes, `2` means the same thing whether
//! you're looking at all pads or a filtered subset. The index is independent of
//! the current query.
//!
//! Some operations (like delete) do alter indexes—but this matches user expectations.
//! If you see items 1-5 and delete #2, you expect the list to shrink to 4 items.
//!
//! ## Index Notation
//!
//! - **Regular**: `1`, `2`, `3`... — All non-deleted pads, newest first
//! - **Pinned**: `p1`, `p2`, `p3`... — Pinned pads only, for quick access
//! - **Deleted**: `d1`, `d2`, `d3`... — Soft-deleted pads (for recovery/purge)
//!
//! ## Dual Indexing for Pinned Pads
//!
//! **Pinned pads appear in BOTH the pinned list AND the regular list.**
//!
//! A pinned pad has two valid indexes:
//! - `p1` (its pinned index)
//! - `2` (its regular index, based on creation time)
//!
//! Why? **Canonical stability.** A pad's regular index doesn't change when pinned
//! or unpinned. Users can always refer to pad `3` as `3`, regardless of pin status.
//!
//! ## Example
//!
//! Given 4 pads (newest to oldest): A (pinned), B, C, D (deleted)
//!
//! ```text
//! p1. A          ← Pinned section
//!
//! 1. A           ← Regular section (A appears again!)
//! 2. B
//! 3. C
//!
//! d1. D          ← Deleted section (only with --deleted flag)
//! ```
//!
//! User can refer to pad A as either `p1` or `1`.

use crate::model::Pad;

/// A user-facing index for a pad.
#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct DisplayPad {
    pub pad: Pad,
    pub index: DisplayIndex,
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
}
