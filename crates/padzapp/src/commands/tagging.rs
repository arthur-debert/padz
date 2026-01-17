//! Pad tagging commands.
//!
//! This module provides operations for managing tags on pads:
//! - `add_tags`: Add tags to pads
//! - `remove_tags`: Remove specific tags from pads
//! - `clear_tags`: Remove all tags from pads

use crate::attributes::AttrValue;
use crate::commands::helpers::{indexed_pads, resolve_selectors};
use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;

/// Add tags to selected pads.
///
/// Tags must exist in the registry before they can be added to pads.
/// Adding a tag that a pad already has is a no-op (idempotent).
pub fn add_tags<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    tags: &[String],
) -> Result<CmdResult> {
    if tags.is_empty() {
        return Err(PadzError::Api("No tags specified".to_string()));
    }

    // Verify all tags exist in the registry
    let registry = store.load_tags(scope)?;
    let registry_names: Vec<&str> = registry.iter().map(|t| t.name.as_str()).collect();
    for tag in tags {
        if !registry_names.contains(&tag.as_str()) {
            return Err(PadzError::Api(format!(
                "Tag '{}' not found. Create it first with 'padz tags create {}'",
                tag, tag
            )));
        }
    }

    let resolved = resolve_selectors(store, scope, selectors, false)?;
    let mut result = CmdResult::default();
    let mut modified_count = 0;

    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope)?;

        // Get current tags via attribute API
        let current_tags = pad
            .metadata
            .get_attr("tags")
            .and_then(|v| v.as_list().map(|l| l.to_vec()))
            .unwrap_or_default();
        let original_count = current_tags.len();

        // Add tags that aren't already present
        let mut new_tags = current_tags;
        for tag in tags {
            if !new_tags.contains(tag) {
                new_tags.push(tag.clone());
            }
        }

        // Sort and save if changed
        if new_tags.len() > original_count {
            new_tags.sort();
            pad.metadata.set_attr("tags", AttrValue::List(new_tags));
            store.save_pad(&pad, scope)?;
            modified_count += 1;
        }

        // Re-index to get updated pad with correct index
        let indexed = indexed_pads(store, scope)?;
        if let Some(dp) = find_pad_by_uuid_any(&indexed, uuid) {
            result.affected_pads.push(DisplayPad {
                pad: dp.pad.clone(),
                index: display_index
                    .last()
                    .cloned()
                    .unwrap_or(DisplayIndex::Regular(1)),
                matches: None,
                children: Vec::new(),
            });
        }
    }

    let tag_list = tags.join(", ");
    if modified_count > 0 {
        result.add_message(CmdMessage::success(format!(
            "Added tag{} [{}] to {} pad{}",
            if tags.len() == 1 { "" } else { "s" },
            tag_list,
            modified_count,
            if modified_count == 1 { "" } else { "s" }
        )));
    } else {
        result.add_message(CmdMessage::info(format!(
            "All pads already have tag{} [{}]",
            if tags.len() == 1 { "" } else { "s" },
            tag_list
        )));
    }

    Ok(result)
}

/// Remove specific tags from selected pads.
///
/// Removing a tag that a pad doesn't have is a no-op (idempotent).
pub fn remove_tags<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    tags: &[String],
) -> Result<CmdResult> {
    if tags.is_empty() {
        return Err(PadzError::Api("No tags specified".to_string()));
    }

    let resolved = resolve_selectors(store, scope, selectors, false)?;
    let mut result = CmdResult::default();
    let mut modified_count = 0;

    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope)?;

        // Get current tags via attribute API
        let current_tags = pad
            .metadata
            .get_attr("tags")
            .and_then(|v| v.as_list().map(|l| l.to_vec()))
            .unwrap_or_default();
        let original_count = current_tags.len();

        // Remove specified tags
        let new_tags: Vec<String> = current_tags
            .into_iter()
            .filter(|t| !tags.contains(t))
            .collect();

        // Save if changed
        if new_tags.len() < original_count {
            pad.metadata.set_attr("tags", AttrValue::List(new_tags));
            store.save_pad(&pad, scope)?;
            modified_count += 1;
        }

        // Re-index to get updated pad with correct index
        let indexed = indexed_pads(store, scope)?;
        if let Some(dp) = find_pad_by_uuid_any(&indexed, uuid) {
            result.affected_pads.push(DisplayPad {
                pad: dp.pad.clone(),
                index: display_index
                    .last()
                    .cloned()
                    .unwrap_or(DisplayIndex::Regular(1)),
                matches: None,
                children: Vec::new(),
            });
        }
    }

    let tag_list = tags.join(", ");
    if modified_count > 0 {
        result.add_message(CmdMessage::success(format!(
            "Removed tag{} [{}] from {} pad{}",
            if tags.len() == 1 { "" } else { "s" },
            tag_list,
            modified_count,
            if modified_count == 1 { "" } else { "s" }
        )));
    } else {
        result.add_message(CmdMessage::info(format!(
            "No pads had tag{} [{}]",
            if tags.len() == 1 { "" } else { "s" },
            tag_list
        )));
    }

    Ok(result)
}

/// Remove all tags from selected pads.
pub fn clear_tags<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
) -> Result<CmdResult> {
    let resolved = resolve_selectors(store, scope, selectors, false)?;
    let mut result = CmdResult::default();
    let mut modified_count = 0;

    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope)?;

        // Get current tags via attribute API
        let current_tags = pad
            .metadata
            .get_attr("tags")
            .and_then(|v| v.as_list().map(|l| l.to_vec()))
            .unwrap_or_default();

        if !current_tags.is_empty() {
            pad.metadata.set_attr("tags", AttrValue::List(Vec::new()));
            store.save_pad(&pad, scope)?;
            modified_count += 1;
        }

        // Re-index to get updated pad with correct index
        let indexed = indexed_pads(store, scope)?;
        if let Some(dp) = find_pad_by_uuid_any(&indexed, uuid) {
            result.affected_pads.push(DisplayPad {
                pad: dp.pad.clone(),
                index: display_index
                    .last()
                    .cloned()
                    .unwrap_or(DisplayIndex::Regular(1)),
                matches: None,
                children: Vec::new(),
            });
        }
    }

    if modified_count > 0 {
        result.add_message(CmdMessage::success(format!(
            "Cleared tags from {} pad{}",
            modified_count,
            if modified_count == 1 { "" } else { "s" }
        )));
    } else {
        result.add_message(CmdMessage::info("No pads had any tags to clear"));
    }

    Ok(result)
}

/// Find a pad by UUID in the indexed tree (any index type).
fn find_pad_by_uuid_any(pads: &[DisplayPad], uuid: uuid::Uuid) -> Option<&DisplayPad> {
    for dp in pads {
        if dp.pad.metadata.id == uuid {
            return Some(dp);
        }
        if let Some(found) = find_pad_by_uuid_any(&dp.children, uuid) {
            return Some(found);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, tags};
    use crate::store::memory::InMemoryStore;

    fn setup_store_with_tag() -> InMemoryStore {
        let mut store = InMemoryStore::new();
        tags::create_tag(&mut store, Scope::Project, "work").unwrap();
        tags::create_tag(&mut store, Scope::Project, "rust").unwrap();
        store
    }

    #[test]
    fn test_add_tags_single_pad() {
        let mut store = setup_store_with_tag();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        let result = add_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["work".to_string()],
        )
        .unwrap();

        assert!(result.messages[0].content.contains("Added tag"));
        assert_eq!(result.affected_pads.len(), 1);
        assert!(result.affected_pads[0]
            .pad
            .metadata
            .tags
            .contains(&"work".to_string()));
    }

    #[test]
    fn test_add_tags_multiple_tags() {
        let mut store = setup_store_with_tag();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        let result = add_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["work".to_string(), "rust".to_string()],
        )
        .unwrap();

        assert!(result.messages[0].content.contains("Added tags"));
        let tags = &result.affected_pads[0].pad.metadata.tags;
        assert!(tags.contains(&"work".to_string()));
        assert!(tags.contains(&"rust".to_string()));
    }

    #[test]
    fn test_add_tags_nonexistent_tag() {
        let mut store = setup_store_with_tag();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        let result = add_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["nonexistent".to_string()],
        );

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_add_tags_idempotent() {
        let mut store = setup_store_with_tag();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        add_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["work".to_string()],
        )
        .unwrap();

        // Adding same tag again
        let result = add_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["work".to_string()],
        )
        .unwrap();

        assert!(result.messages[0].content.contains("already have"));
    }

    #[test]
    fn test_remove_tags() {
        let mut store = setup_store_with_tag();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        add_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["work".to_string()],
        )
        .unwrap();

        let result = remove_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["work".to_string()],
        )
        .unwrap();

        assert!(result.messages[0].content.contains("Removed tag"));
        assert!(result.affected_pads[0].pad.metadata.tags.is_empty());
    }

    #[test]
    fn test_remove_tags_not_present() {
        let mut store = setup_store_with_tag();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        let result = remove_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["work".to_string()],
        )
        .unwrap();

        assert!(result.messages[0].content.contains("No pads had"));
    }

    #[test]
    fn test_clear_tags() {
        let mut store = setup_store_with_tag();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        add_tags(
            &mut store,
            Scope::Project,
            &selectors,
            &["work".to_string(), "rust".to_string()],
        )
        .unwrap();

        let result = clear_tags(&mut store, Scope::Project, &selectors).unwrap();

        assert!(result.messages[0].content.contains("Cleared tags"));
        assert!(result.affected_pads[0].pad.metadata.tags.is_empty());
    }

    #[test]
    fn test_clear_tags_empty() {
        let mut store = setup_store_with_tag();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        let result = clear_tags(&mut store, Scope::Project, &selectors).unwrap();

        assert!(result.messages[0].content.contains("No pads had any tags"));
    }

    #[test]
    fn test_add_tags_no_tags_error() {
        let mut store = setup_store_with_tag();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        let result = add_tags(&mut store, Scope::Project, &selectors, &[]);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No tags specified"));
    }
}
