//! Semantic tag-registry operations.
//!
//! Registry order and affected-pad counts are application facts. Human prose,
//! pluralization, line layout, and styling belong to clients.

use crate::error::{PadzError, Result};
use crate::model::Scope;
use crate::store::{Bucket, DataStore};
use crate::tags::{validate_tag_name, TagEntry};

/// An ordered catalog of tag names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagCatalogOutcome {
    /// No tags are defined for the requested registry or pad selection.
    Empty,
    /// Tags in the order the command contract exposes them.
    Listed { tags: Vec<String> },
}

impl TagCatalogOutcome {
    fn from_names(tags: Vec<String>) -> Self {
        if tags.is_empty() {
            Self::Empty
        } else {
            Self::Listed { tags }
        }
    }
}

/// A completed tag-registry mutation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagRegistryOutcome {
    Created {
        name: String,
        affected_pads: usize,
    },
    Deleted {
        name: String,
        affected_pads: usize,
    },
    Renamed {
        old_name: String,
        new_name: String,
        affected_pads: usize,
    },
}

/// List all tags in registry order.
pub fn list_tags<S: DataStore>(store: &S, scope: Scope) -> Result<TagCatalogOutcome> {
    let tags = store
        .load_tags(scope)?
        .into_iter()
        .map(|tag| tag.name)
        .collect();
    Ok(TagCatalogOutcome::from_names(tags))
}

/// List the unique tags on selected pads in lexical order.
///
/// The lexical ordering preserves the established `tag list <id>...` contract.
pub fn list_pad_tags<S: DataStore>(
    store: &S,
    scope: Scope,
    selectors: &[crate::index::PadSelector],
) -> Result<TagCatalogOutcome> {
    use crate::commands::helpers::{resolve_selectors, TitleBucket};
    use std::collections::BTreeSet;

    let resolved = resolve_selectors(store, scope, selectors, false, TitleBucket::Active)?;
    let mut all_tags = BTreeSet::new();

    for (_display_index, uuid) in resolved {
        let pad = store.get_pad(&uuid, scope, Bucket::Active)?;
        all_tags.extend(pad.metadata.tags);
    }

    Ok(TagCatalogOutcome::from_names(
        all_tags.into_iter().collect(),
    ))
}

/// Create a new tag in the registry.
///
/// Returns an error if the tag name is invalid or already exists. Creation does
/// not modify any pads, so the semantic affected count is always zero.
pub fn create_tag<S: DataStore>(
    store: &mut S,
    scope: Scope,
    name: &str,
) -> Result<TagRegistryOutcome> {
    validate_tag_name(name).map_err(|e| PadzError::Api(e.to_string()))?;

    let mut tags = store.load_tags(scope)?;
    if tags.iter().any(|tag| tag.name == name) {
        return Err(PadzError::Api(format!("Tag '{}' already exists", name)));
    }

    tags.push(TagEntry::new(name.to_string()));
    store.save_tags(scope, &tags)?;

    Ok(TagRegistryOutcome::Created {
        name: name.to_string(),
        affected_pads: 0,
    })
}

/// Delete a registry tag and remove it from every active pad that carries it.
pub fn delete_tag<S: DataStore>(
    store: &mut S,
    scope: Scope,
    name: &str,
) -> Result<TagRegistryOutcome> {
    let mut tags = store.load_tags(scope)?;
    let original_len = tags.len();
    tags.retain(|tag| tag.name != name);

    if tags.len() == original_len {
        return Err(PadzError::Api(format!("Tag '{}' not found", name)));
    }

    store.save_tags(scope, &tags)?;

    let mut affected_pads = 0;
    for mut pad in store.list_pads(scope, Bucket::Active)? {
        if pad.metadata.tags.iter().any(|tag| tag == name) {
            pad.metadata.tags.retain(|tag| tag != name);
            store.save_pad(&pad, scope, Bucket::Active)?;
            affected_pads += 1;
        }
    }

    Ok(TagRegistryOutcome::Deleted {
        name: name.to_string(),
        affected_pads,
    })
}

/// Rename a registry tag and update every active pad that carries it.
pub fn rename_tag<S: DataStore>(
    store: &mut S,
    scope: Scope,
    old_name: &str,
    new_name: &str,
) -> Result<TagRegistryOutcome> {
    validate_tag_name(new_name).map_err(|e| PadzError::Api(e.to_string()))?;

    let mut tags = store.load_tags(scope)?;
    let tag_idx = tags
        .iter()
        .position(|tag| tag.name == old_name)
        .ok_or_else(|| PadzError::Api(format!("Tag '{}' not found", old_name)))?;

    if tags.iter().any(|tag| tag.name == new_name) {
        return Err(PadzError::Api(format!("Tag '{}' already exists", new_name)));
    }

    tags[tag_idx].name = new_name.to_string();
    store.save_tags(scope, &tags)?;

    let mut affected_pads = 0;
    for mut pad in store.list_pads(scope, Bucket::Active)? {
        if let Some(pos) = pad.metadata.tags.iter().position(|tag| tag == old_name) {
            pad.metadata.tags[pos] = new_name.to_string();
            store.save_pad(&pad, scope, Bucket::Active)?;
            affected_pads += 1;
        }
    }

    Ok(TagRegistryOutcome::Renamed {
        old_name: old_name.to_string(),
        new_name: new_name.to_string(),
        affected_pads,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, tagging};
    use crate::index::{DisplayIndex, PadSelector};
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

    #[test]
    fn empty_registry_is_an_explicit_catalog_state() {
        assert_eq!(
            list_tags(&store(), Scope::Project).unwrap(),
            TagCatalogOutcome::Empty
        );
    }

    #[test]
    fn registry_catalog_preserves_creation_order() {
        let mut store = store();
        create_tag(&mut store, Scope::Project, "work").unwrap();
        create_tag(&mut store, Scope::Project, "rust").unwrap();

        assert_eq!(
            list_tags(&store, Scope::Project).unwrap(),
            TagCatalogOutcome::Listed {
                tags: vec!["work".into(), "rust".into()]
            }
        );
    }

    #[test]
    fn create_returns_the_name_and_zero_affected_pads() {
        let mut store = store();

        assert_eq!(
            create_tag(&mut store, Scope::Project, "work").unwrap(),
            TagRegistryOutcome::Created {
                name: "work".into(),
                affected_pads: 0,
            }
        );
        assert_eq!(store.load_tags(Scope::Project).unwrap()[0].name, "work");
    }

    #[test]
    fn create_preserves_invalid_and_duplicate_errors() {
        let mut store = store();
        assert!(create_tag(&mut store, Scope::Project, "-invalid").is_err());
        create_tag(&mut store, Scope::Project, "work").unwrap();
        let error = create_tag(&mut store, Scope::Project, "work").unwrap_err();
        assert!(error.to_string().contains("already exists"));
    }

    #[test]
    fn delete_returns_its_name_and_cascade_count() {
        let mut store = store();
        create_tag(&mut store, Scope::Project, "work").unwrap();
        create::run(&mut store, Scope::Project, "One".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Two".into(), "".into(), None).unwrap();
        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[
                PadSelector::Path(vec![DisplayIndex::Regular(1)]),
                PadSelector::Path(vec![DisplayIndex::Regular(2)]),
            ],
            &["work".into()],
        )
        .unwrap();

        assert_eq!(
            delete_tag(&mut store, Scope::Project, "work").unwrap(),
            TagRegistryOutcome::Deleted {
                name: "work".into(),
                affected_pads: 2,
            }
        );
        assert!(store
            .list_pads(Scope::Project, Bucket::Active)
            .unwrap()
            .iter()
            .all(|pad| pad.metadata.tags.is_empty()));
    }

    #[test]
    fn delete_preserves_not_found_error() {
        let error = delete_tag(&mut store(), Scope::Project, "missing").unwrap_err();
        assert!(error.to_string().contains("not found"));
    }

    #[test]
    fn rename_returns_both_names_and_update_count() {
        let mut store = store();
        create_tag(&mut store, Scope::Project, "old").unwrap();
        create::run(&mut store, Scope::Project, "Pad".into(), "".into(), None).unwrap();
        tagging::add_tags(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            &["old".into()],
        )
        .unwrap();

        assert_eq!(
            rename_tag(&mut store, Scope::Project, "old", "new").unwrap(),
            TagRegistryOutcome::Renamed {
                old_name: "old".into(),
                new_name: "new".into(),
                affected_pads: 1,
            }
        );
        assert_eq!(
            store.list_pads(Scope::Project, Bucket::Active).unwrap()[0]
                .metadata
                .tags,
            vec!["new"]
        );
    }

    #[test]
    fn rename_preserves_validation_duplicate_and_not_found_errors() {
        let mut store = store();
        create_tag(&mut store, Scope::Project, "one").unwrap();
        create_tag(&mut store, Scope::Project, "two").unwrap();

        assert!(rename_tag(&mut store, Scope::Project, "one", "-invalid").is_err());
        assert!(rename_tag(&mut store, Scope::Project, "one", "two")
            .unwrap_err()
            .to_string()
            .contains("already exists"));
        assert!(rename_tag(&mut store, Scope::Project, "missing", "three")
            .unwrap_err()
            .to_string()
            .contains("not found"));
    }

    #[test]
    fn pad_catalog_is_unique_and_lexically_ordered() {
        let mut store = store();
        create::run(&mut store, Scope::Project, "Pad".into(), "".into(), None).unwrap();
        let selectors = [PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        tagging::add_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["work".into(), "rust".into()],
        )
        .unwrap();

        assert_eq!(
            list_pad_tags(&store, Scope::Project, &selectors).unwrap(),
            TagCatalogOutcome::Listed {
                tags: vec!["rust".into(), "work".into()]
            }
        );
    }

    #[test]
    fn untagged_pad_has_an_explicit_empty_catalog() {
        let mut store = store();
        create::run(&mut store, Scope::Project, "Pad".into(), "".into(), None).unwrap();
        let selectors = [PadSelector::Path(vec![DisplayIndex::Regular(1)])];

        assert_eq!(
            list_pad_tags(&store, Scope::Project, &selectors).unwrap(),
            TagCatalogOutcome::Empty
        );
    }
}
