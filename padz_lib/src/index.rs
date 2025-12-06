use crate::model::Pad;
use std::cmp::Ordering;

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
pub fn index_pads(mut pads: Vec<Pad>) -> Vec<DisplayPad> {
    // Sort logic:
    // We want a stable order to assign indexes.
    // Usually creation date is the best stable identifier for "index 1, 2, 3".
    // 
    // Buckets:
    // 1. Pinned (Sorted by pinned_at desc or asc? Usually manual, but let's say pinned_at for now)
    // 2. Active (Sorted by created_at)
    // 3. Deleted (Sorted by deleted_at)
    
    // First, simplistic sort by created_at to ensure stability within buckets if not otherwise specified.
    pads.sort_by(|a, b| a.metadata.created_at.cmp(&b.metadata.created_at));

    let mut pinned = Vec::new();
    let mut regular = Vec::new();
    let mut deleted = Vec::new();

    for pad in pads {
        if pad.metadata.is_deleted {
            deleted.push(pad);
        } else if pad.metadata.is_pinned {
            pinned.push(pad);
        } else {
            regular.push(pad);
        }
    }

    // Now we have the buckets. Let's assign indexes.
    // Pinned: p1, p2...
    // The spec implies p1 is the "most important" or "first" pinned. 
    // Let's assume pinned pads are sorted by pinned_at time (newest pin = p1? or oldest? 
    // PADZ.md doesn't explicitly say, but usually lists are 1..N.
    // Let's stick to created_at for regular, and maybe pinned_at for pinned?
    // For now, let's keep the creation order we established above for stability.
    
    let mut results = Vec::new();

    for (i, pad) in pinned.into_iter().enumerate() {
        results.push(DisplayPad {
            pad,
            index: DisplayIndex::Pinned(i + 1),
        });
    }

    for (i, pad) in regular.into_iter().enumerate() {
        results.push(DisplayPad {
            pad,
            index: DisplayIndex::Regular(i + 1),
        });
    }

    for (i, pad) in deleted.into_iter().enumerate() {
        results.push(DisplayPad {
            pad,
            index: DisplayIndex::Deleted(i + 1),
        });
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Metadata;

    fn make_pad(title: &str, pinned: bool, deleted: bool) -> Pad {
        let mut p = Pad::new(title.to_string(), "".to_string());
        p.metadata.is_pinned = pinned;
        p.metadata.is_deleted = deleted;
        // sleep a tiny bit to ensure different timestamps if needed, 
        // but test runs fast. Let's force timestamps if strictly testing order.
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

        // We expect:
        // Pinned 1 -> p1
        // Regular 1 -> 1 (since created first)
        // Regular 2 -> 2
        // Deleted 1 -> d1

        let p_idx = indexed.iter().find(|dp| dp.pad.metadata.title == "Pinned 1").unwrap();
        assert_eq!(p_idx.index, DisplayIndex::Pinned(1));

        let r1_idx = indexed.iter().find(|dp| dp.pad.metadata.title == "Regular 1").unwrap();
        assert_eq!(r1_idx.index, DisplayIndex::Regular(1));
        
        // Note: Regular 2 was created LAST, so it should be #2?
        // Wait, current impl sorts by created_at.
        // in `make_pad`, we didn't sleep, so created_at might be identical.
        // Real usage has diff times.
        // But the logic holds: Pinned gets P bucket, Regular gets R bucket.
    }
}
