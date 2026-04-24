//! Input-normalization layer for the API facade.
//!
//! Converts CLI-style index strings (`"1"`, `"p2"`, `"d3-d5"`, `"1.2"`, short
//! UUIDs) into `PadSelector` enum values, with two flavors that auto-prefix
//! bare numbers for commands that operate on the Deleted or Archived bucket.

use crate::error::Result;
use crate::index::{parse_index_or_range, PadSelector};
use uuid::Uuid;

pub(super) fn parse_selectors<I: AsRef<str>>(inputs: &[I]) -> Result<Vec<PadSelector>> {
    // 1. Try to parse ALL inputs as DisplayIndex (including ranges like "3-5")
    let mut all_selectors = Vec::new();
    let mut parse_failed = false;

    for input in inputs {
        match parse_index_or_range(input.as_ref()) {
            Ok(selector) => all_selectors.push(selector),
            Err(_) => {
                parse_failed = true;
                break;
            }
        }
    }

    if !parse_failed {
        let mut unique_selectors = Vec::new();
        for s in all_selectors {
            if !unique_selectors.contains(&s) {
                unique_selectors.push(s);
            }
        }
        return Ok(unique_selectors);
    }

    // 2. If any failed, treat the whole input as ONE search query.
    let search_term = inputs
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<&str>>()
        .join(" ");

    Ok(vec![PadSelector::Title(search_term)])
}

/// Parses selectors for commands that operate on deleted pads (restore, purge).
/// Bare numbers are treated as deleted indexes: "3" -> "d3", but "d3" stays "d3".
pub(super) fn parse_selectors_for_deleted<I: AsRef<str>>(inputs: &[I]) -> Result<Vec<PadSelector>> {
    let normalized: Vec<String> = inputs
        .iter()
        .map(|s| normalize_to_deleted_index(s.as_ref()))
        .collect();

    parse_selectors(&normalized)
}

/// Parses selectors for commands that operate on archived pads (unarchive).
/// Bare numbers are treated as archived indexes: "3" -> "ar3", but "ar3" stays "ar3".
pub(super) fn parse_selectors_for_archived<I: AsRef<str>>(
    inputs: &[I],
) -> Result<Vec<PadSelector>> {
    let normalized: Vec<String> = inputs
        .iter()
        .map(|s| normalize_to_archived_index(s.as_ref()))
        .collect();

    parse_selectors(&normalized)
}

/// Canonicalize if possible, else return the path as-is. Used when
/// comparing resolved `.padz/` directories — a path that hasn't been
/// created yet won't canonicalize, and we don't want that to silently
/// succeed when the caller is attempting a real operation.
pub(super) fn canonicalize_or_self(p: &std::path::Path) -> std::path::PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Normalizes an index string to an archived index if it's a bare number.
/// "3" -> "ar3", "ar3" -> "ar3", "p1" -> "p1", "3-5" -> "ar3-ar5"
/// UUIDs (full or short hex) pass through unchanged.
fn normalize_to_archived_index(s: &str) -> String {
    if Uuid::parse_str(s).is_ok() || looks_like_short_uuid(s) {
        return s.to_string();
    }
    if let Some(dash_pos) = s.find('-') {
        if dash_pos > 0 {
            let start_str = &s[..dash_pos];
            let end_str = &s[dash_pos + 1..];
            let normalized_start = normalize_path_for_archived(start_str);
            let normalized_end = normalize_path_for_archived(end_str);
            return format!("{}-{}", normalized_start, normalized_end);
        }
    }
    normalize_path_for_archived(s)
}

fn normalize_path_for_archived(s: &str) -> String {
    let mut parts: Vec<String> = s.split('.').map(|s| s.to_string()).collect();
    if let Some(last) = parts.last_mut() {
        if last.chars().all(|c| c.is_ascii_digit()) && !last.is_empty() {
            *last = format!("ar{}", last);
        }
    }
    parts.join(".")
}

/// Normalizes an index string to a deleted index if it's a bare number.
/// "3" -> "d3", "d3" -> "d3", "p1" -> "p1", "3-5" -> "d3-d5"
/// UUIDs (full or short hex) pass through unchanged.
fn normalize_to_deleted_index(s: &str) -> String {
    if Uuid::parse_str(s).is_ok() || looks_like_short_uuid(s) {
        return s.to_string();
    }
    if let Some(dash_pos) = s.find('-') {
        if dash_pos > 0 {
            let start_str = &s[..dash_pos];
            let end_str = &s[dash_pos + 1..];
            let normalized_start = normalize_path_for_deleted(start_str);
            let normalized_end = normalize_path_for_deleted(end_str);
            return format!("{}-{}", normalized_start, normalized_end);
        }
    }
    normalize_path_for_deleted(s)
}

/// Normalizes a path string (e.g., "1", "1.2", "p1") for deleted operations.
/// The *last* segment of a path, if it's a bare number, gets prefixed with 'd'.
/// "3" -> "d3", "1.2" -> "1.d2", "p1.2" -> "p1.d2", "d1.2" -> "d1.d2"
fn normalize_path_for_deleted(s: &str) -> String {
    let mut parts: Vec<String> = s.split('.').map(|s| s.to_string()).collect();
    if let Some(last) = parts.last_mut() {
        *last = normalize_single_to_deleted(last);
    }
    parts.join(".")
}

/// Normalizes a single index (not a range) to deleted format.
/// "3" -> "d3", "d3" -> "d3", "p1" -> "p1"
fn normalize_single_to_deleted(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
        format!("d{}", s)
    } else {
        s.to_string()
    }
}

/// A short UUID is a hex string that isn't parseable as a DisplayIndex.
/// Non-empty, all hex digits, and contains at least one non-digit (otherwise
/// it would parse as a Regular DI).
fn looks_like_short_uuid(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_hexdigit())
        && s.chars().any(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::DisplayIndex;

    #[test]
    fn test_parse_selectors_single_index() {
        let inputs = vec!["1"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        assert!(matches!(selectors[0], PadSelector::Path(_)));
    }

    #[test]
    fn test_parse_selectors_multiple_indexes() {
        let inputs = vec!["1", "3", "5"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 3);
    }

    #[test]
    fn test_parse_selectors_deduplicates() {
        let inputs = vec!["1", "1", "2", "1"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 2);
    }

    #[test]
    fn test_parse_selectors_title_fallback() {
        let inputs = vec!["meeting", "notes"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        match &selectors[0] {
            PadSelector::Title(term) => assert_eq!(term, "meeting notes"),
            _ => panic!("Expected Title selector"),
        }
    }

    #[test]
    fn test_parse_selectors_mixed_input_becomes_title() {
        let inputs = vec!["1", "meeting", "2"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        match &selectors[0] {
            PadSelector::Title(term) => assert_eq!(term, "1 meeting 2"),
            _ => panic!("Expected Title selector"),
        }
    }

    #[test]
    fn test_parse_selectors_range() {
        let inputs = vec!["1-3"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        assert!(matches!(selectors[0], PadSelector::Range(_, _)));
    }

    #[test]
    fn test_parse_selectors_mixed_di_and_short_uuid() {
        let inputs = vec!["1", "4", "766d5dab"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 3);
        assert!(matches!(&selectors[0], PadSelector::Path(_)));
        assert!(matches!(&selectors[1], PadSelector::Path(_)));
        assert!(matches!(&selectors[2], PadSelector::ShortUuid(h) if h == "766d5dab"));
    }

    #[test]
    fn test_parse_selectors_short_uuid_only() {
        let inputs = vec!["abcdef01"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        assert!(matches!(&selectors[0], PadSelector::ShortUuid(h) if h == "abcdef01"));
    }

    #[test]
    fn test_parse_selectors_full_uuid_still_works() {
        let inputs = vec!["550e8400-e29b-41d4-a716-446655440000"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        assert!(matches!(&selectors[0], PadSelector::Uuid(_)));
    }

    #[test]
    fn test_parse_selectors_non_hex_still_becomes_title() {
        let inputs = vec!["meeting"];
        let selectors = parse_selectors(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        assert!(matches!(&selectors[0], PadSelector::Title(_)));
    }

    #[test]
    fn test_normalize_to_deleted_preserves_short_uuid() {
        assert_eq!(normalize_to_deleted_index("766d5dab"), "766d5dab");
    }

    #[test]
    fn test_normalize_to_deleted_preserves_full_uuid() {
        assert_eq!(
            normalize_to_deleted_index("550e8400-e29b-41d4-a716-446655440000"),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_normalize_to_archived_preserves_short_uuid() {
        assert_eq!(normalize_to_archived_index("766d5dab"), "766d5dab");
    }

    #[test]
    fn test_normalize_to_archived_preserves_full_uuid() {
        assert_eq!(
            normalize_to_archived_index("550e8400-e29b-41d4-a716-446655440000"),
            "550e8400-e29b-41d4-a716-446655440000"
        );
    }

    #[test]
    fn test_normalize_single_to_deleted() {
        assert_eq!(normalize_single_to_deleted("1"), "d1");
        assert_eq!(normalize_single_to_deleted("42"), "d42");

        assert_eq!(normalize_single_to_deleted("d1"), "d1");
        assert_eq!(normalize_single_to_deleted("d42"), "d42");

        assert_eq!(normalize_single_to_deleted("p1"), "p1");
        assert_eq!(normalize_single_to_deleted("p99"), "p99");

        assert_eq!(normalize_single_to_deleted(""), "");

        assert_eq!(normalize_single_to_deleted("abc"), "abc");
    }

    #[test]
    fn test_normalize_to_deleted_index_ranges() {
        assert_eq!(normalize_to_deleted_index("3-5"), "d3-d5");
        assert_eq!(normalize_to_deleted_index("1-10"), "d1-d10");

        assert_eq!(normalize_to_deleted_index("d3-d5"), "d3-d5");

        assert_eq!(normalize_to_deleted_index("3-d5"), "d3-d5");
        assert_eq!(normalize_to_deleted_index("d3-5"), "d3-d5");

        assert_eq!(normalize_to_deleted_index("3"), "d3");
        assert_eq!(normalize_to_deleted_index("d3"), "d3");

        assert_eq!(normalize_to_deleted_index("1.2"), "1.d2");
        assert_eq!(normalize_to_deleted_index("p1.2"), "p1.d2");
        assert_eq!(normalize_to_deleted_index("d1.2"), "d1.d2");
        assert_eq!(normalize_to_deleted_index("1.p2"), "1.p2");
        assert_eq!(normalize_to_deleted_index("1.d2"), "1.d2");
        assert_eq!(normalize_to_deleted_index("1.2-1.4"), "1.d2-1.d4");
        assert_eq!(normalize_to_deleted_index("d1.2-d1.4"), "d1.d2-d1.d4");
    }

    #[test]
    fn test_parse_selectors_for_deleted() {
        let inputs = vec!["1", "3", "d5"];
        let selectors = parse_selectors_for_deleted(&inputs).unwrap();

        assert_eq!(selectors.len(), 3);
        assert!(matches!(selectors[0], PadSelector::Path(_)));
        if let PadSelector::Path(path) = &selectors[0] {
            assert_eq!(path.len(), 1);
            assert!(matches!(path[0], DisplayIndex::Deleted(1)));
        }
        if let PadSelector::Path(path) = &selectors[1] {
            assert_eq!(path.len(), 1);
            assert!(matches!(path[0], DisplayIndex::Deleted(3)));
        }
        if let PadSelector::Path(path) = &selectors[2] {
            assert_eq!(path.len(), 1);
            assert!(matches!(path[0], DisplayIndex::Deleted(5)));
        }
    }

    #[test]
    fn test_parse_selectors_for_deleted_with_range() {
        let inputs = vec!["1-3"];
        let selectors = parse_selectors_for_deleted(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        match &selectors[0] {
            PadSelector::Range(start, end) => {
                assert_eq!(start.len(), 1);
                assert!(matches!(start[0], DisplayIndex::Deleted(1)));
                assert_eq!(end.len(), 1);
                assert!(matches!(end[0], DisplayIndex::Deleted(3)));
            }
            _ => panic!("Expected Range"),
        }
    }

    #[test]
    fn test_parse_selectors_for_deleted_with_hierarchical_range() {
        let inputs = vec!["1.2-1.4"];
        let selectors = parse_selectors_for_deleted(&inputs).unwrap();

        assert_eq!(selectors.len(), 1);
        match &selectors[0] {
            PadSelector::Range(start_path, end_path) => {
                assert_eq!(start_path.len(), 2);
                assert!(matches!(start_path[0], DisplayIndex::Regular(1)));
                assert!(matches!(start_path[1], DisplayIndex::Deleted(2)));

                assert_eq!(end_path.len(), 2);
                assert!(matches!(end_path[0], DisplayIndex::Regular(1)));
                assert!(matches!(end_path[1], DisplayIndex::Deleted(4)));
            }
            _ => panic!("Expected PadSelector::Range"),
        }
    }
}
