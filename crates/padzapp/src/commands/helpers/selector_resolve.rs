use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;
use uuid::Uuid;

use super::fmt_path;
use super::indexing::indexed_pads;

/// Bucket scope for `PadSelector::Title` matching.
///
/// Other selector variants (Path, Range, Uuid, ShortUuid) carry their bucket
/// information intrinsically and are unaffected by this filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TitleBucket {
    /// Only match pads in the active bucket (Regular + Pinned roots).
    Active,
    /// Only match pads in the archived bucket.
    Archived,
    /// Only match pads in the deleted bucket.
    Deleted,
    /// Match across all buckets.
    Any,
}

/// Maximum number of ambiguous matches to list inline in the error message.
/// Above this, we fall back to just reporting the count.
const AMBIGUITY_LIST_THRESHOLD: usize = 5;

pub fn resolve_selectors<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    check_delete_protection: bool,
    title_bucket: TitleBucket,
) -> Result<Vec<(Vec<DisplayIndex>, Uuid)>> {
    let root_pads = indexed_pads(store, scope)?;

    let linearized = linearize_tree(&root_pads);

    let mut results = Vec::new();

    for selector in selectors {
        match selector {
            PadSelector::Path(path) => {
                if let Some((_, pad)) = find_in_linearized(&linearized, path) {
                    check_protection(pad, check_delete_protection)?;
                    results.push((path.clone(), pad.pad.metadata.id));
                } else {
                    let s: Vec<String> = path.iter().map(|idx| idx.to_string()).collect();
                    return Err(PadzError::Api(format!(
                        "Index {} not found in current scope",
                        s.join(".")
                    )));
                }
            }
            PadSelector::Range(start_path, end_path) => {
                let start_idx = linearized
                    .iter()
                    .position(|(p, _)| p == start_path)
                    .ok_or_else(|| {
                        PadzError::Api(format!("Range start {} not found", fmt_path(start_path)))
                    })?;
                let end_idx = linearized
                    .iter()
                    .position(|(p, _)| p == end_path)
                    .ok_or_else(|| {
                        PadzError::Api(format!("Range end {} not found", fmt_path(end_path)))
                    })?;

                if start_idx > end_idx {
                    return Err(PadzError::Api(format!(
                        "Invalid range: {} appears after {} in the list",
                        fmt_path(start_path),
                        fmt_path(end_path)
                    )));
                }

                for (path, pad) in linearized.iter().take(end_idx + 1).skip(start_idx) {
                    check_protection(pad, check_delete_protection)?;
                    results.push((path.clone(), pad.pad.metadata.id));
                }
            }
            PadSelector::Uuid(uuid) => {
                let found = linearized
                    .iter()
                    .find(|(_, dp)| dp.pad.metadata.id == *uuid);

                match found {
                    Some((path, dp)) => {
                        check_protection(dp, check_delete_protection)?;
                        results.push((path.clone(), dp.pad.metadata.id));
                    }
                    None => {
                        return Err(PadzError::Api(format!("No pad found with UUID {}", uuid)));
                    }
                }
            }
            PadSelector::ShortUuid(hex) => {
                let matches: Vec<&(Vec<DisplayIndex>, &DisplayPad)> = linearized
                    .iter()
                    .filter(|(_, dp)| {
                        dp.pad
                            .metadata
                            .id
                            .to_string()
                            .replace('-', "")
                            .starts_with(hex.as_str())
                    })
                    .collect();

                match matches.len() {
                    0 => {
                        return Err(PadzError::Api(format!(
                            "No pad found with UUID prefix {}",
                            hex
                        )));
                    }
                    1 => {
                        let (path, dp) = matches[0];
                        check_protection(dp, check_delete_protection)?;
                        results.push((path.clone(), dp.pad.metadata.id));
                    }
                    n => {
                        return Err(PadzError::Api(format!(
                            "UUID prefix \"{}\" matches {} pads. Use more characters to be unique.",
                            hex, n
                        )));
                    }
                }
            }
            PadSelector::Title(term) => {
                let term_lower = term.to_lowercase();
                let matches: Vec<&(Vec<DisplayIndex>, &DisplayPad)> = linearized
                    .iter()
                    .filter(|(path, _)| path_in_bucket(path, title_bucket))
                    .filter(|(_, dp)| dp.pad.metadata.title.to_lowercase().contains(&term_lower))
                    .collect();

                match matches.len() {
                    0 => {
                        return Err(PadzError::Api(format!(
                            "No pad found matching \"{}\"",
                            term
                        )))
                    }
                    1 => {
                        let (path, dp) = matches[0];
                        check_protection(dp, check_delete_protection)?;
                        results.push((path.clone(), dp.pad.metadata.id));
                    }
                    n if n <= AMBIGUITY_LIST_THRESHOLD => {
                        let listing: String = matches
                            .iter()
                            .map(|(path, dp)| {
                                format!(
                                    "    {}. {}",
                                    style_index(&fmt_path(path)),
                                    style_title_with_match(&dp.pad.metadata.title, &term_lower)
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        return Err(PadzError::Api(format!(
                            "Term {} matches multiple pads. Use one, or be more specific:\n{}",
                            style_match(term),
                            listing
                        )));
                    }
                    n => {
                        return Err(PadzError::Api(format!(
                            "Term {} matches {} pads. Please be more specific.",
                            style_match(term),
                            n
                        )));
                    }
                }
            }
        }
    }

    Ok(results)
}

/// Format a display index in the same accent color the list/search renderer
/// uses for `list-index` (gold/yellow). When stderr is not a TTY or the
/// terminal can't take color, `console::style` collapses to plain text on its
/// own — no extra `IsTerminal` checks needed here.
fn style_index(s: &str) -> String {
    console::style(s).yellow().to_string()
}

/// Format the search term with the same yellow-background highlight the list/
/// search renderer uses for `match` hits (yellow bg, black fg). Wraps the
/// styled term in quotes so the message reads `Term "foo" matches ...`.
fn style_match(term: &str) -> String {
    format!("\"{}\"", console::style(term).black().on_yellow())
}

/// Render `title` plain, except for the substring matching `term_lower` (case-
/// insensitive), which gets the same yellow-background highlight as search hits.
fn style_title_with_match(title: &str, term_lower: &str) -> String {
    if term_lower.is_empty() {
        return title.to_string();
    }
    let title_lower = title.to_lowercase();
    let mut out = String::with_capacity(title.len() + 16);
    let mut cursor = 0usize;
    while let Some(rel) = title_lower[cursor..].find(term_lower) {
        let start = cursor + rel;
        let end = start + term_lower.len();
        out.push_str(&title[cursor..start]);
        out.push_str(
            &console::style(&title[start..end])
                .black()
                .on_yellow()
                .to_string(),
        );
        cursor = end;
    }
    out.push_str(&title[cursor..]);
    out
}

/// Is this pad in the bucket we're filtering to?
///
/// A pad's bucket is determined by its *own* local index (the last segment
/// of its path), matching `bucket_for_index`. This correctly handles nested
/// cases like a deleted child under an active parent (path `7.d3`), which
/// should count as deleted, not active.
///
/// If *any* ancestor segment is Archived or Deleted, treat the pad as living
/// in that ancestor's bucket too — an active child of an archived parent is
/// not reachable as "active" for selection purposes.
fn path_in_bucket(path: &[DisplayIndex], bucket: TitleBucket) -> bool {
    if bucket == TitleBucket::Any {
        return true;
    }
    if path.is_empty() {
        return false;
    }
    // A path lives in Deleted if any segment is Deleted; similarly for Archived.
    // Otherwise it's Active (all segments are Regular/Pinned).
    let has_deleted = path.iter().any(|i| matches!(i, DisplayIndex::Deleted(_)));
    let has_archived = path.iter().any(|i| matches!(i, DisplayIndex::Archived(_)));
    match bucket {
        TitleBucket::Deleted => has_deleted,
        TitleBucket::Archived => has_archived && !has_deleted,
        TitleBucket::Active => !has_deleted && !has_archived,
        TitleBucket::Any => true,
    }
}

fn check_protection(dp: &DisplayPad, enabled: bool) -> Result<()> {
    if enabled && dp.pad.metadata.delete_protected {
        return Err(PadzError::Api(
            "Pinned pads are delete protected, unpin then delete it".to_string(),
        ));
    }
    Ok(())
}

fn linearize_tree(roots: &[DisplayPad]) -> Vec<(Vec<DisplayIndex>, &DisplayPad)> {
    let mut result = Vec::new();
    for pad in roots {
        linearize_recursive(pad, Vec::new(), &mut result);
    }
    result
}

fn linearize_recursive<'a>(
    pad: &'a DisplayPad,
    parent_path: Vec<DisplayIndex>,
    result: &mut Vec<(Vec<DisplayIndex>, &'a DisplayPad)>,
) {
    let mut current_path = parent_path;
    current_path.push(pad.index.clone());

    result.push((current_path.clone(), pad));

    for child in &pad.children {
        linearize_recursive(child, current_path.clone(), result);
    }
}

fn find_in_linearized<'a>(
    linearized: &'a [(Vec<DisplayIndex>, &'a DisplayPad)],
    path: &[DisplayIndex],
) -> Option<&'a (Vec<DisplayIndex>, &'a DisplayPad)> {
    linearized.iter().find(|(p, _)| p == path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::index::DisplayIndex;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;
    use crate::store::Bucket;

    #[test]
    fn test_range_selection_within_siblings() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child A".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child B".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child C".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1), DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(1), DisplayIndex::Regular(3)],
            )],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_range_selection_cross_parent() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(
            &mut store,
            Scope::Project,
            "Parent 1".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child 1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        create::run(
            &mut store,
            Scope::Project,
            "Parent 2".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child 2".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1), DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(2), DisplayIndex::Regular(1)],
            )],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_range_selection_root_only() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Root 1".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child 1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(&mut store, Scope::Project, "Root 2".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(2)],
            )],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_range_includes_children_of_intermediate_nodes() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Root 1".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child 1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(&mut store, Scope::Project, "Root 2".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Root 3".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(3), DisplayIndex::Regular(1)],
            )],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(result.len(), 4);
    }

    #[test]
    fn test_pinned_child_addressable_by_path() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        crate::commands::pinning::pin(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Pinned(1),
            ])],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "Child");
    }

    #[test]
    fn test_title_search_no_match_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Alpha".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Beta".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Gamma".to_string())],
            false,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("No pad found matching"));
        assert!(err.to_string().contains("Gamma"));
    }

    #[test]
    fn test_title_search_multiple_matches_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Meeting Monday".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Meeting Tuesday".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Meeting Wednesday".into(),
            "".into(),
            None,
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Meeting".to_string())],
            false,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // Strip ANSI styling so substring assertions are stable regardless of
        // whether `console` decided to emit colors in this environment.
        let plain = console::strip_ansi_codes(&err).to_string();
        assert!(plain.contains("multiple pads"));
        // Under the listing threshold, matches are enumerated by title.
        assert!(plain.contains("Meeting Monday"));
        assert!(plain.contains("Meeting Tuesday"));
        assert!(plain.contains("Meeting Wednesday"));
    }

    #[test]
    fn test_title_search_single_match_succeeds() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Alpha".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Beta".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Gamma".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Beta".to_string())],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "Beta");
    }

    #[test]
    fn test_title_search_matches_title_only_not_content() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Shopping List".into(),
            "Buy apples and oranges".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Todo List".into(),
            "Call dentist".into(),
            None,
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("apples".to_string())],
            false,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No pad found"));

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Shopping".to_string())],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "Shopping List");
    }

    #[test]
    fn test_title_search_is_case_insensitive() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "UPPERCASE TITLE".into(),
            "".into(),
            None,
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("uppercase".to_string())],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "UPPERCASE TITLE");
    }

    #[test]
    fn test_title_search_delete_protection_check() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "ProtectedPad".into(),
            "".into(),
            None,
        )
        .unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads[0].clone();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("ProtectedPad".to_string())],
            true,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }

    #[test]
    fn test_title_search_delete_protection_disabled() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "ProtectedPad".into(),
            "".into(),
            None,
        )
        .unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads[0].clone();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("ProtectedPad".to_string())],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_uuid_resolution() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result =
            create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        let pad_uuid = result.affected_pads[0].pad.metadata.id;

        let resolved = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Uuid(pad_uuid)],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].1, pad_uuid);
    }

    #[test]
    fn test_uuid_not_found_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        let fake_uuid = uuid::Uuid::new_v4();
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Uuid(fake_uuid)],
            false,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("No pad found with UUID"));
    }

    #[test]
    fn test_uuid_delete_protection() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result =
            create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        let pad_uuid = result.affected_pads[0].pad.metadata.id;

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads[0].clone();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Uuid(pad_uuid)],
            true,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }

    #[test]
    fn test_range_invalid_order_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad B".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad C".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(3)],
                vec![DisplayIndex::Regular(1)],
            )],
            false,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("appears after"));
    }

    #[test]
    fn test_range_start_not_found_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(99)],
                vec![DisplayIndex::Regular(1)],
            )],
            false,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Range start"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_range_end_not_found_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(99)],
            )],
            false,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Range end"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_range_delete_protection_check() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad B".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad C".into(), "".into(), None).unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad_b = pads
            .iter()
            .find(|p| p.metadata.title == "Pad B")
            .unwrap()
            .clone();
        pad_b.metadata.delete_protected = true;
        store
            .save_pad(&pad_b, Scope::Project, Bucket::Active)
            .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(3)],
            )],
            true,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }

    #[test]
    fn test_path_selector_not_found_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(99)])],
            false,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_path_selector_delete_protection_check() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads[0].clone();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            true,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }

    #[test]
    fn test_short_uuid_resolves_to_pad() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result =
            create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        let pad_uuid = result.affected_pads[0].pad.metadata.id;

        let hex = pad_uuid.to_string().replace('-', "");
        let short = &hex[..8];

        let resolved = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::ShortUuid(short.to_string())],
            false,
            TitleBucket::Any,
        )
        .unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].1, pad_uuid);
    }

    #[test]
    fn test_short_uuid_not_found() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::ShortUuid("00000000".to_string())],
            false,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("No pad found with UUID prefix"));
    }

    #[test]
    fn test_short_uuid_delete_protection() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result =
            create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        let pad_uuid = result.affected_pads[0].pad.metadata.id;

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let mut pad = pads[0].clone();
        pad.metadata.delete_protected = true;
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        let hex = pad_uuid.to_string().replace('-', "");
        let short = &hex[..8];

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::ShortUuid(short.to_string())],
            true,
            TitleBucket::Any,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }

    // --- TitleBucket scoping -------------------------------------------------

    #[test]
    fn test_title_bucket_active_ignores_deleted_matches() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        // Active pad (only one with "for" in the title that should count)
        create::run(
            &mut store,
            Scope::Project,
            "Task for Padz".into(),
            "".into(),
            None,
        )
        .unwrap();
        // A second active pad whose title does NOT match — used as a target to delete.
        create::run(
            &mut store,
            Scope::Project,
            "Feature Flag for Ids".into(),
            "".into(),
            None,
        )
        .unwrap();
        // Send the second one to deleted.
        crate::commands::delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        // With Active scope, only the one active match should resolve.
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("for".to_string())],
            false,
            TitleBucket::Active,
        )
        .unwrap();
        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "Task for Padz");

        // With Any scope, both the active and deleted titles match — ambiguous.
        let any_result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("for".to_string())],
            false,
            TitleBucket::Any,
        );
        assert!(any_result.is_err());
    }

    #[test]
    fn test_title_bucket_deleted_matches_only_deleted() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Active Note".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Trash Note".into(),
            "".into(),
            None,
        )
        .unwrap();
        // The first-created pad is now at index 2 (newest first); delete the
        // "Trash Note" which is at index 1.
        crate::commands::delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Note".to_string())],
            false,
            TitleBucket::Deleted,
        )
        .unwrap();
        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Deleted)
            .unwrap();
        assert_eq!(pad.metadata.title, "Trash Note");
    }

    #[test]
    fn test_title_bucket_archived_matches_only_archived() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Active Shared".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Archive Shared".into(),
            "".into(),
            None,
        )
        .unwrap();
        // Archive "Archive Shared" (index 1 after creation).
        crate::commands::archive::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap();

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Shared".to_string())],
            false,
            TitleBucket::Archived,
        )
        .unwrap();
        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Archived)
            .unwrap();
        assert_eq!(pad.metadata.title, "Archive Shared");
    }

    #[test]
    fn test_title_bucket_active_excludes_deleted_child_under_active_parent() {
        // Regression for the real-world case where a deleted child under an active
        // parent (path like `7.d3`) was counted as an active match because only
        // the root segment was inspected.
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        // Active parent.
        create::run(
            &mut store,
            Scope::Project,
            "Parent Folder".into(),
            "".into(),
            None,
        )
        .unwrap();
        // Two children under the active parent.
        create::run(
            &mut store,
            Scope::Project,
            "Keep Note".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Trash Note".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();
        // Delete the second child. Its path becomes `1.d1` (deleted child under
        // an active parent).
        crate::commands::delete::run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
        )
        .unwrap();

        // Active-scoped title match for "Note" should only return the surviving
        // "Keep Note" — the deleted "Trash Note" must be excluded.
        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Note".to_string())],
            false,
            TitleBucket::Active,
        )
        .unwrap();
        assert_eq!(result.len(), 1);
        let pad = store
            .get_pad(&result[0].1, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(pad.metadata.title, "Keep Note");
    }

    #[test]
    fn test_title_ambiguity_over_threshold_reports_count_only() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        // Create more than AMBIGUITY_LIST_THRESHOLD (=5) matches.
        for i in 1..=6 {
            create::run(
                &mut store,
                Scope::Project,
                format!("Meeting {}", i),
                "".into(),
                None,
            )
            .unwrap();
        }

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Meeting".to_string())],
            false,
            TitleBucket::Active,
        );
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        // Over the threshold we fall back to a count-only error, without enumerating titles.
        assert!(err.contains("6 pads"));
        assert!(!err.contains("Meeting 1"));
    }
}
