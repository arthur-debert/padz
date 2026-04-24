use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;
use uuid::Uuid;

use super::fmt_path;
use super::indexing::indexed_pads;

pub fn resolve_selectors<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[PadSelector],
    check_delete_protection: bool,
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
                    .filter(|(_, dp)| dp.pad.metadata.title.to_lowercase().contains(&term_lower))
                    .collect();

                match matches.len() {
                    0 => return Err(PadzError::Api(format!("No pad found matching \"{}\"", term))),
                    1 => {
                        let (path, dp) = matches[0];
                        check_protection(dp, check_delete_protection)?;
                        results.push((path.clone(), dp.pad.metadata.id));
                    },
                    n => return Err(PadzError::Api(format!(
                        "Term \"{}\" matches multiple paths, add more to make it unique(matched {} pads). Please be more specific.",
                        term, n
                    ))),
                }
            }
        }
    }

    Ok(results)
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
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("multiple"));
        assert!(err.to_string().contains("3"));
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
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No pad found"));

        let result = resolve_selectors(
            &store,
            Scope::Project,
            &[PadSelector::Title("Shopping".to_string())],
            false,
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
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("delete protected"));
    }
}
