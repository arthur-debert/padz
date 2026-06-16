use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, DisplayPad, PadSelector};

/// Filters the tree to only include pads that match the given selectors.
/// Each matched pad is returned with its full subtree of children.
pub(super) fn filter_by_selectors(
    pads: Vec<DisplayPad>,
    selectors: &[PadSelector],
) -> Result<Vec<DisplayPad>> {
    let linearized = linearize_for_filter(&pads);
    let mut matched = Vec::new();

    for selector in selectors {
        match selector {
            PadSelector::Path(path) => {
                if let Some(dp) = find_by_path(&linearized, path) {
                    if !matched
                        .iter()
                        .any(|m: &DisplayPad| m.pad.metadata.id == dp.pad.metadata.id)
                    {
                        matched.push(dp.clone());
                    }
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
                        let s: Vec<String> = start_path.iter().map(|idx| idx.to_string()).collect();
                        PadzError::Api(format!("Range start {} not found", s.join(".")))
                    })?;
                let end_idx = linearized
                    .iter()
                    .position(|(p, _)| p == end_path)
                    .ok_or_else(|| {
                        let s: Vec<String> = end_path.iter().map(|idx| idx.to_string()).collect();
                        PadzError::Api(format!("Range end {} not found", s.join(".")))
                    })?;

                if start_idx > end_idx {
                    return Err(PadzError::Api(
                        "Invalid range: start appears after end".into(),
                    ));
                }

                for (_, dp) in linearized.iter().take(end_idx + 1).skip(start_idx) {
                    if !matched
                        .iter()
                        .any(|m: &DisplayPad| m.pad.metadata.id == dp.pad.metadata.id)
                    {
                        matched.push((*dp).clone());
                    }
                }
            }
            PadSelector::Uuid(uuid) => {
                let found = linearized
                    .iter()
                    .find(|(_, dp)| dp.pad.metadata.id == *uuid);

                match found {
                    Some((_, dp)) => {
                        if !matched
                            .iter()
                            .any(|m: &DisplayPad| m.pad.metadata.id == dp.pad.metadata.id)
                        {
                            matched.push((*dp).clone());
                        }
                    }
                    None => {
                        return Err(PadzError::Api(format!("No pad found with UUID {}", uuid)));
                    }
                }
            }
            PadSelector::ShortUuid(hex) => {
                let matches: Vec<&&DisplayPad> = linearized
                    .iter()
                    .filter(|(_, dp)| {
                        dp.pad
                            .metadata
                            .id
                            .to_string()
                            .replace('-', "")
                            .starts_with(hex.as_str())
                    })
                    .map(|(_, dp)| dp)
                    .collect();

                match matches.len() {
                    0 => {
                        return Err(PadzError::Api(format!(
                            "No pad found with UUID prefix {}",
                            hex
                        )));
                    }
                    1 => {
                        if !matched
                            .iter()
                            .any(|m: &DisplayPad| m.pad.metadata.id == matches[0].pad.metadata.id)
                        {
                            matched.push((*matches[0]).clone());
                        }
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
                let matches: Vec<&DisplayPad> = linearized
                    .iter()
                    .filter(|(_, dp)| dp.pad.metadata.title.to_lowercase().contains(&term_lower))
                    .map(|(_, dp)| *dp)
                    .collect();

                if matches.is_empty() {
                    return Err(PadzError::Api(format!(
                        "No pad found matching \"{}\"",
                        term
                    )));
                }

                for dp in matches {
                    if !matched
                        .iter()
                        .any(|m: &DisplayPad| m.pad.metadata.id == dp.pad.metadata.id)
                    {
                        matched.push(dp.clone());
                    }
                }
            }
        }
    }

    Ok(matched)
}

/// Linearize the tree into (path, &DisplayPad) pairs for selector resolution.
fn linearize_for_filter(roots: &[DisplayPad]) -> Vec<(Vec<DisplayIndex>, &DisplayPad)> {
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

fn find_by_path<'a>(
    linearized: &[(Vec<DisplayIndex>, &'a DisplayPad)],
    path: &[DisplayIndex],
) -> Option<&'a DisplayPad> {
    linearized
        .iter()
        .find(|(p, _)| p == path)
        .map(|(_, dp)| *dp)
}

#[cfg(test)]
mod tests {
    use crate::commands::create;
    use crate::commands::get::{run, PadFilter};
    use crate::index::{DisplayIndex, PadSelector};
    use crate::model::Scope;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    fn fresh_store() -> BucketedStore<MemBackend> {
        BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        )
    }

    #[test]
    fn test_id_selector_single_pad() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "First".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Second".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Third".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Path(vec![DisplayIndex::Regular(2)])],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Second");
    }

    #[test]
    fn test_id_selector_multiple_pads() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "First".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Second".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Third".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(3)]),
            ],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);
        let titles: Vec<_> = res
            .listed_pads
            .iter()
            .map(|dp| dp.pad.metadata.title.as_str())
            .collect();
        assert!(titles.contains(&"Third"));
        assert!(titles.contains(&"First"));
    }

    #[test]
    fn test_id_selector_with_children() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "Parent1".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Parent2".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child1".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(2)])),
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Child2".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(2)])),
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Path(vec![DisplayIndex::Regular(2)])],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Parent1");
        assert_eq!(res.listed_pads[0].children.len(), 2);
    }

    #[test]
    fn test_id_selector_range() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "First".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Second".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Third".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Fourth".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(2)],
                vec![DisplayIndex::Regular(3)],
            )],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);
    }

    #[test]
    fn test_id_selector_not_found() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "Only".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Path(vec![DisplayIndex::Regular(5)])],
        );

        assert!(res.is_err());
    }

    #[test]
    fn test_id_selector_preserves_index() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(&mut store, Scope::Project, "First".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Second".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Third".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Path(vec![DisplayIndex::Regular(3)])],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert!(matches!(res.listed_pads[0].index, DisplayIndex::Regular(3)));
    }

    // ------- UUID selector tests -------

    #[test]
    fn test_uuid_selector_matches_pad() {
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "Alpha".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Bravo".into(), "".into(), None).unwrap();

        // Discover the UUID via an unfiltered listing
        let all = run(&store, Scope::Project, PadFilter::default(), &[]).unwrap();
        let bravo = all
            .listed_pads
            .iter()
            .find(|dp| dp.pad.metadata.title == "Bravo")
            .expect("Bravo should be listed");
        let bravo_uuid = bravo.pad.metadata.id;

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Uuid(bravo_uuid)],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Bravo");
    }

    #[test]
    fn test_uuid_selector_not_found_errors() {
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "Solo".into(), "".into(), None).unwrap();

        let bogus = uuid::Uuid::nil();
        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Uuid(bogus)],
        );

        let err = res.unwrap_err().to_string();
        assert!(err.contains("No pad found with UUID"), "got: {err}");
    }

    // ------- ShortUuid selector tests -------

    #[test]
    fn test_short_uuid_selector_matches_pad() {
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "First".into(), "".into(), None).unwrap();

        let all = run(&store, Scope::Project, PadFilter::default(), &[]).unwrap();
        let dp = &all.listed_pads[0];
        // First 8 hex digits of the UUID make a very-unlikely-to-collide prefix.
        let short: String = dp.pad.metadata.id.to_string().replace('-', "")[..8].to_string();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::ShortUuid(short)],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "First");
    }

    #[test]
    fn test_short_uuid_selector_no_match_errors() {
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "Only".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            // "zzzzzz" can never appear in a hex UUID.
            &[PadSelector::ShortUuid("zzzzzz".to_string())],
        );

        let err = res.unwrap_err().to_string();
        assert!(err.contains("No pad found with UUID prefix"), "got: {err}");
    }

    #[test]
    fn test_short_uuid_selector_ambiguous_errors() {
        // Using an empty prefix forces ALL pads to match — the ambiguous branch.
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "One".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Two".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::ShortUuid(String::new())],
        );

        let err = res.unwrap_err().to_string();
        assert!(
            err.contains("matches") && err.contains("Use more characters"),
            "got: {err}"
        );
    }

    // ------- Title selector tests -------

    #[test]
    fn test_title_selector_case_insensitive_substring() {
        let mut store = fresh_store();
        create::run(
            &mut store,
            Scope::Project,
            "Quick Brown Fox".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Sleeping Cat".into(),
            "".into(),
            None,
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Title("BROWN".into())],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Quick Brown Fox");
    }

    #[test]
    fn test_title_selector_matches_multiple() {
        let mut store = fresh_store();
        create::run(
            &mut store,
            Scope::Project,
            "todo: groceries".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "todo: laundry".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "unrelated".into(),
            "".into(),
            None,
        )
        .unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Title("todo:".into())],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 2);
    }

    #[test]
    fn test_title_selector_no_match_errors() {
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "Hello".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Title("nonexistent".into())],
        );

        let err = res.unwrap_err().to_string();
        assert!(err.contains("No pad found matching"), "got: {err}");
    }

    // ------- Range edge cases -------

    #[test]
    fn test_range_inverted_start_after_end_errors() {
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "C".into(), "".into(), None).unwrap();

        // Index 1 is the newest ("C"), index 3 is the oldest ("A").
        // Start=1, end=3 is valid; flip it to trigger the inverted error.
        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(3)],
                vec![DisplayIndex::Regular(1)],
            )],
        );

        let err = res.unwrap_err().to_string();
        assert!(
            err.contains("Invalid range") && err.contains("start appears after end"),
            "got: {err}"
        );
    }

    #[test]
    fn test_range_start_not_found_errors() {
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(99)],
                vec![DisplayIndex::Regular(1)],
            )],
        );

        let err = res.unwrap_err().to_string();
        assert!(
            err.contains("Range start") && err.contains("not found"),
            "got: {err}"
        );
    }

    #[test]
    fn test_range_end_not_found_errors() {
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[PadSelector::Range(
                vec![DisplayIndex::Regular(1)],
                vec![DisplayIndex::Regular(99)],
            )],
        );

        let err = res.unwrap_err().to_string();
        assert!(
            err.contains("Range end") && err.contains("not found"),
            "got: {err}"
        );
    }

    // ------- Deduplication -------

    #[test]
    fn test_overlapping_selectors_dedupe() {
        // Selecting the same pad via two different selectors must yield it once.
        let mut store = fresh_store();
        create::run(
            &mut store,
            Scope::Project,
            "Unique-Title".into(),
            "".into(),
            None,
        )
        .unwrap();
        create::run(&mut store, Scope::Project, "Other".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[
                // Index 2 is "Unique-Title" (newest is index 1).
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
                PadSelector::Title("Unique".into()),
            ],
        )
        .unwrap();

        assert_eq!(res.listed_pads.len(), 1);
        assert_eq!(res.listed_pads[0].pad.metadata.title, "Unique-Title");
    }

    #[test]
    fn test_overlapping_ranges_dedupe() {
        let mut store = fresh_store();
        create::run(&mut store, Scope::Project, "First".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Second".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Third".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Fourth".into(), "".into(), None).unwrap();

        let res = run(
            &store,
            Scope::Project,
            PadFilter::default(),
            &[
                PadSelector::Range(
                    vec![DisplayIndex::Regular(1)],
                    vec![DisplayIndex::Regular(3)],
                ),
                PadSelector::Range(
                    vec![DisplayIndex::Regular(2)],
                    vec![DisplayIndex::Regular(4)],
                ),
            ],
        )
        .unwrap();

        // Union of [1..=3] and [2..=4] is [1..=4]; no pad appears twice.
        assert_eq!(res.listed_pads.len(), 4);
    }
}
