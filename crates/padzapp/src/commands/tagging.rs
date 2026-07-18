//! Semantic per-pad tag assignment and removal.
//!
//! Requested tag order, changed-pad counts, and no-op distinctions are domain
//! facts. Human sentences, brackets, pluralization, and styles belong to clients.

use crate::attributes::AttrValue;
use crate::commands::helpers::{indexed_pads, resolve_selectors, TitleBucket};
use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::{Bucket, DataStore};

/// The semantic result of applying or removing requested tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaggingOutcome {
    Assigned {
        requested_tags: Vec<String>,
        modified_pads: usize,
    },
    AllAlreadyPresent {
        requested_tags: Vec<String>,
        modified_pads: usize,
    },
    Removed {
        requested_tags: Vec<String>,
        modified_pads: usize,
    },
    NonePresent {
        requested_tags: Vec<String>,
        modified_pads: usize,
    },
}

/// Selected pads after a tag mutation, paired with its semantic outcome.
#[derive(Debug, Clone)]
pub struct TaggingResult {
    pub affected_pads: Vec<DisplayPad>,
    pub outcome: TaggingOutcome,
}

/// Add tags to selected pads, auto-creating missing registry entries.
///
/// Adding tags that every selected pad already carries is an explicit no-op.
pub fn add_tags<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    tags: &[String],
) -> Result<TaggingResult> {
    if tags.is_empty() {
        return Err(PadzError::Api("No tags specified".to_string()));
    }

    let mut registry = store.load_tags(scope)?;
    let mut registry_changed = false;
    for tag in tags {
        if !registry.iter().any(|entry| entry.name == *tag) {
            use crate::tags::{validate_tag_name, TagEntry};
            validate_tag_name(tag).map_err(|e| PadzError::Api(e.to_string()))?;
            registry.push(TagEntry::new(tag.clone()));
            registry_changed = true;
        }
    }
    if registry_changed {
        store.save_tags(scope, &registry)?;
    }

    let resolved = resolve_selectors(store, scope, selectors, false, TitleBucket::Active)?;
    let mut affected_pads = Vec::with_capacity(resolved.len());
    let mut modified_pads = 0;

    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope, Bucket::Active)?;
        let current_tags = pad
            .metadata
            .get_attr("tags")
            .and_then(|value| value.as_list().map(<[String]>::to_vec))
            .unwrap_or_default();
        let original_count = current_tags.len();

        let mut new_tags = current_tags;
        for tag in tags {
            if !new_tags.contains(tag) {
                new_tags.push(tag.clone());
            }
        }

        if new_tags.len() > original_count {
            new_tags.sort();
            pad.metadata.set_attr("tags", AttrValue::List(new_tags));
            store.save_pad(&pad, scope, Bucket::Active)?;
            modified_pads += 1;
        }

        if let Some(display_pad) = find_pad_by_uuid_any(&indexed_pads(store, scope)?, uuid) {
            affected_pads.push(DisplayPad {
                pad: display_pad.pad.clone(),
                index: display_index
                    .last()
                    .cloned()
                    .unwrap_or(DisplayIndex::Regular(1)),
                matches: None,
                children: Vec::new(),
            });
        }
    }

    let requested_tags = tags.to_vec();
    let outcome = if modified_pads == 0 {
        TaggingOutcome::AllAlreadyPresent {
            requested_tags,
            modified_pads,
        }
    } else {
        TaggingOutcome::Assigned {
            requested_tags,
            modified_pads,
        }
    };

    Ok(TaggingResult {
        affected_pads,
        outcome,
    })
}

/// Remove requested tags from selected pads.
///
/// A request where none of the selected pads carries any requested tag is an
/// explicit no-op. Registry entries are not removed.
pub fn remove_tags<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    tags: &[String],
) -> Result<TaggingResult> {
    if tags.is_empty() {
        return Err(PadzError::Api("No tags specified".to_string()));
    }

    let resolved = resolve_selectors(store, scope, selectors, false, TitleBucket::Active)?;
    let mut affected_pads = Vec::with_capacity(resolved.len());
    let mut modified_pads = 0;

    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope, Bucket::Active)?;
        let current_tags = pad
            .metadata
            .get_attr("tags")
            .and_then(|value| value.as_list().map(<[String]>::to_vec))
            .unwrap_or_default();
        let original_count = current_tags.len();
        let new_tags = current_tags
            .into_iter()
            .filter(|tag| !tags.contains(tag))
            .collect::<Vec<_>>();

        if new_tags.len() < original_count {
            pad.metadata.set_attr("tags", AttrValue::List(new_tags));
            store.save_pad(&pad, scope, Bucket::Active)?;
            modified_pads += 1;
        }

        if let Some(display_pad) = find_pad_by_uuid_any(&indexed_pads(store, scope)?, uuid) {
            affected_pads.push(DisplayPad {
                pad: display_pad.pad.clone(),
                index: display_index
                    .last()
                    .cloned()
                    .unwrap_or(DisplayIndex::Regular(1)),
                matches: None,
                children: Vec::new(),
            });
        }
    }

    let requested_tags = tags.to_vec();
    let outcome = if modified_pads == 0 {
        TaggingOutcome::NonePresent {
            requested_tags,
            modified_pads,
        }
    } else {
        TaggingOutcome::Removed {
            requested_tags,
            modified_pads,
        }
    };

    Ok(TaggingResult {
        affected_pads,
        outcome,
    })
}

fn find_pad_by_uuid_any(pads: &[DisplayPad], uuid: uuid::Uuid) -> Option<&DisplayPad> {
    for pad in pads {
        if pad.pad.metadata.id == uuid {
            return Some(pad);
        }
        if let Some(found) = find_pad_by_uuid_any(&pad.children, uuid) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, tags};
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    fn store() -> BucketedStore<MemBackend> {
        BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        )
    }

    fn selectors(count: usize) -> Vec<PadSelector> {
        (1..=count)
            .map(|index| PadSelector::Path(vec![DisplayIndex::Regular(index)]))
            .collect()
    }

    #[test]
    fn assigning_one_tag_returns_requested_tags_and_modified_pad_count() {
        let mut store = store();
        create::run(&mut store, Scope::Project, "Pad".into(), "".into(), None).unwrap();

        let result = add_tags(&mut store, Scope::Project, &selectors(1), &["work".into()]).unwrap();

        assert_eq!(
            result.outcome,
            TaggingOutcome::Assigned {
                requested_tags: vec!["work".into()],
                modified_pads: 1,
            }
        );
        assert_eq!(result.affected_pads[0].pad.metadata.tags, vec!["work"]);
        assert!(store
            .load_tags(Scope::Project)
            .unwrap()
            .iter()
            .any(|tag| tag.name == "work"));
    }

    #[test]
    fn assigning_multiple_tags_counts_pads_not_tag_applications() {
        let mut store = store();
        create::run(&mut store, Scope::Project, "One".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Two".into(), "".into(), None).unwrap();
        add_tags(
            &mut store,
            Scope::Project,
            &selectors(1),
            &["work".into(), "rust".into()],
        )
        .unwrap();

        let result = add_tags(
            &mut store,
            Scope::Project,
            &selectors(2),
            &["work".into(), "rust".into()],
        )
        .unwrap();

        assert_eq!(
            result.outcome,
            TaggingOutcome::Assigned {
                requested_tags: vec!["work".into(), "rust".into()],
                modified_pads: 1,
            }
        );
        assert_eq!(result.affected_pads.len(), 2);
    }

    #[test]
    fn repeated_assignment_is_a_distinct_all_already_present_no_op() {
        let mut store = store();
        create::run(&mut store, Scope::Project, "Pad".into(), "".into(), None).unwrap();
        add_tags(&mut store, Scope::Project, &selectors(1), &["work".into()]).unwrap();

        let result = add_tags(&mut store, Scope::Project, &selectors(1), &["work".into()]).unwrap();

        assert_eq!(
            result.outcome,
            TaggingOutcome::AllAlreadyPresent {
                requested_tags: vec!["work".into()],
                modified_pads: 0,
            }
        );
        assert_eq!(result.affected_pads.len(), 1);
    }

    #[test]
    fn removing_tags_returns_requested_tags_and_modified_pad_count() {
        let mut store = store();
        create::run(&mut store, Scope::Project, "Pad".into(), "".into(), None).unwrap();
        add_tags(
            &mut store,
            Scope::Project,
            &selectors(1),
            &["work".into(), "rust".into()],
        )
        .unwrap();

        let result = remove_tags(
            &mut store,
            Scope::Project,
            &selectors(1),
            &["work".into(), "rust".into()],
        )
        .unwrap();

        assert_eq!(
            result.outcome,
            TaggingOutcome::Removed {
                requested_tags: vec!["work".into(), "rust".into()],
                modified_pads: 1,
            }
        );
        assert!(result.affected_pads[0].pad.metadata.tags.is_empty());
    }

    #[test]
    fn removing_absent_tags_is_a_distinct_none_present_no_op() {
        let mut store = store();
        tags::create_tag(&mut store, Scope::Project, "work").unwrap();
        create::run(&mut store, Scope::Project, "Pad".into(), "".into(), None).unwrap();

        let result =
            remove_tags(&mut store, Scope::Project, &selectors(1), &["work".into()]).unwrap();

        assert_eq!(
            result.outcome,
            TaggingOutcome::NonePresent {
                requested_tags: vec!["work".into()],
                modified_pads: 0,
            }
        );
        assert_eq!(result.affected_pads.len(), 1);
    }

    #[test]
    fn empty_tag_requests_preserve_validation_error() {
        let mut store = store();
        create::run(&mut store, Scope::Project, "Pad".into(), "".into(), None).unwrap();
        let error = add_tags(&mut store, Scope::Project, &selectors(1), &[]).unwrap_err();
        assert!(error.to_string().contains("No tags specified"));
    }

    #[test]
    fn invalid_auto_created_tag_preserves_validation_error_without_registry_write() {
        let mut store = store();
        create::run(&mut store, Scope::Project, "Pad".into(), "".into(), None).unwrap();

        let error = add_tags(
            &mut store,
            Scope::Project,
            &selectors(1),
            &["-invalid".into()],
        )
        .unwrap_err();

        assert!(error.to_string().contains("must start"));
        assert!(store.load_tags(Scope::Project).unwrap().is_empty());
    }
}
