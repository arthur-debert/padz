use crate::model::Pad;

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

/// Takes a raw list of pads and assigns canonical indexes.
/// This sort order MUST be stable.
///
/// Important: Pinned pads appear in BOTH the pinned list (p1, p2...) AND
/// the regular list (1, 2...). This ensures canonical indexes are stable
/// across views - a pad always has the same regular index whether pinned or not.
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
        assert!(
            note_b_entries
                .iter()
                .any(|dp| dp.index == DisplayIndex::Pinned(1))
        );
        // One should be Regular(2) - it's the second newest pad
        assert!(
            note_b_entries
                .iter()
                .any(|dp| dp.index == DisplayIndex::Regular(2))
        );
    }
}
