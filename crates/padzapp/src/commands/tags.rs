//! Tag management commands.
//!
//! This module provides CRUD operations for the tag registry:
//! - `list_tags`: List all tags in a scope
//! - `create_tag`: Create a new tag
//! - `delete_tag`: Delete a tag (cascades to pads)
//! - `rename_tag`: Rename a tag (updates all pads)

use crate::commands::{CmdMessage, CmdResult};
use crate::error::{PadzError, Result};
use crate::model::Scope;
use crate::store::Bucket;
use crate::store::DataStore;
use crate::tags::{validate_tag_name, TagEntry};

/// List all tags in the registry.
pub fn list_tags<S: DataStore>(store: &S, scope: Scope) -> Result<CmdResult> {
    let tags = store.load_tags(scope)?;
    let mut result = CmdResult::default();

    if tags.is_empty() {
        result.add_message(CmdMessage::info("No tags defined"));
    } else {
        let count = tags.len();
        result.add_message(CmdMessage::info(format!(
            "{} tag{} defined",
            count,
            if count == 1 { "" } else { "s" }
        )));
    }

    // Store tags in a way the CLI can render them
    // For now, we'll use messages to list them
    for tag in &tags {
        result.add_message(CmdMessage::info(format!("  {}", tag.name)));
    }

    Ok(result)
}

/// Create a new tag in the registry.
///
/// Returns an error if the tag name is invalid or already exists.
pub fn create_tag<S: DataStore>(store: &mut S, scope: Scope, name: &str) -> Result<CmdResult> {
    // Validate tag name
    validate_tag_name(name).map_err(|e| PadzError::Api(e.to_string()))?;

    // Check if tag already exists
    let mut tags = store.load_tags(scope)?;
    if tags.iter().any(|t| t.name == name) {
        return Err(PadzError::Api(format!("Tag '{}' already exists", name)));
    }

    // Create and save the tag
    let tag = TagEntry::new(name.to_string());
    tags.push(tag);
    store.save_tags(scope, &tags)?;

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!("Created tag '{}'", name)));
    Ok(result)
}

/// Delete a tag from the registry.
///
/// This cascades to all pads that have the tag - the tag is removed from their metadata.
pub fn delete_tag<S: DataStore>(store: &mut S, scope: Scope, name: &str) -> Result<CmdResult> {
    let mut tags = store.load_tags(scope)?;

    // Find and remove the tag
    let original_len = tags.len();
    tags.retain(|t| t.name != name);

    if tags.len() == original_len {
        return Err(PadzError::Api(format!("Tag '{}' not found", name)));
    }

    // Save updated tag registry
    store.save_tags(scope, &tags)?;

    // Cascade: remove tag from all pads
    let pads = store.list_pads(scope, Bucket::Active)?;
    let mut affected_count = 0;
    for mut pad in pads {
        if pad.metadata.tags.contains(&name.to_string()) {
            pad.metadata.tags.retain(|t| t != name);
            store.save_pad(&pad, scope, Bucket::Active)?;
            affected_count += 1;
        }
    }

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!("Deleted tag '{}'", name)));
    if affected_count > 0 {
        result.add_message(CmdMessage::info(format!(
            "Removed from {} pad{}",
            affected_count,
            if affected_count == 1 { "" } else { "s" }
        )));
    }
    Ok(result)
}

/// Rename a tag in the registry.
///
/// This updates all pads that have the old tag name to use the new name.
pub fn rename_tag<S: DataStore>(
    store: &mut S,
    scope: Scope,
    old_name: &str,
    new_name: &str,
) -> Result<CmdResult> {
    // Validate new tag name
    validate_tag_name(new_name).map_err(|e| PadzError::Api(e.to_string()))?;

    let mut tags = store.load_tags(scope)?;

    // Check if old tag exists
    let tag_idx = tags
        .iter()
        .position(|t| t.name == old_name)
        .ok_or_else(|| PadzError::Api(format!("Tag '{}' not found", old_name)))?;

    // Check if new name already exists
    if tags.iter().any(|t| t.name == new_name) {
        return Err(PadzError::Api(format!("Tag '{}' already exists", new_name)));
    }

    // Update the tag name in the registry
    tags[tag_idx].name = new_name.to_string();
    store.save_tags(scope, &tags)?;

    // Update all pads that have this tag
    let pads = store.list_pads(scope, Bucket::Active)?;
    let mut affected_count = 0;
    for mut pad in pads {
        if let Some(pos) = pad.metadata.tags.iter().position(|t| t == old_name) {
            pad.metadata.tags[pos] = new_name.to_string();
            store.save_pad(&pad, scope, Bucket::Active)?;
            affected_count += 1;
        }
    }

    let mut result = CmdResult::default();
    result.add_message(CmdMessage::success(format!(
        "Renamed tag '{}' to '{}'",
        old_name, new_name
    )));
    if affected_count > 0 {
        result.add_message(CmdMessage::info(format!(
            "Updated {} pad{}",
            affected_count,
            if affected_count == 1 { "" } else { "s" }
        )));
    }
    Ok(result)
}

/// Ensure a tag exists in the registry, creating it if needed.
///
/// This is idempotent: if the tag already exists, it's a no-op.
/// Returns an error only if the tag name is invalid.
pub fn ensure_tag<S: DataStore>(store: &mut S, scope: Scope, name: &str) -> Result<()> {
    validate_tag_name(name).map_err(|e| PadzError::Api(e.to_string()))?;

    let mut tags = store.load_tags(scope)?;
    if !tags.iter().any(|t| t.name == name) {
        tags.push(TagEntry::new(name.to_string()));
        store.save_tags(scope, &tags)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn test_list_tags_empty() {
        let store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result = list_tags(&store, Scope::Project).unwrap();
        assert!(result.messages[0].content.contains("No tags defined"));
    }

    #[test]
    fn test_create_tag() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result = create_tag(&mut store, Scope::Project, "work").unwrap();
        assert!(result.messages[0].content.contains("Created tag 'work'"));

        // Verify tag exists
        let tags = store.load_tags(Scope::Project).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "work");
    }

    #[test]
    fn test_create_tag_invalid_name() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result = create_tag(&mut store, Scope::Project, "-invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_create_tag_duplicate() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create_tag(&mut store, Scope::Project, "work").unwrap();
        let result = create_tag(&mut store, Scope::Project, "work");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_delete_tag() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create_tag(&mut store, Scope::Project, "work").unwrap();

        let result = delete_tag(&mut store, Scope::Project, "work").unwrap();
        assert!(result.messages[0].content.contains("Deleted tag 'work'"));

        // Verify tag is gone
        let tags = store.load_tags(Scope::Project).unwrap();
        assert!(tags.is_empty());
    }

    #[test]
    fn test_delete_tag_not_found() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result = delete_tag(&mut store, Scope::Project, "nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_delete_tag_cascades_to_pads() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create a tag and a pad with that tag
        create_tag(&mut store, Scope::Project, "work").unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Test Pad".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        // Manually add tag to pad
        let mut pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        pads[0].metadata.tags.push("work".to_string());
        store
            .save_pad(&pads[0], Scope::Project, Bucket::Active)
            .unwrap();

        // Delete the tag
        let result = delete_tag(&mut store, Scope::Project, "work").unwrap();
        assert!(result.messages[1].content.contains("Removed from 1 pad"));

        // Verify tag is removed from pad
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert!(pads[0].metadata.tags.is_empty());
    }

    #[test]
    fn test_rename_tag() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create_tag(&mut store, Scope::Project, "old-name").unwrap();

        let result = rename_tag(&mut store, Scope::Project, "old-name", "new-name").unwrap();
        assert!(result.messages[0]
            .content
            .contains("Renamed tag 'old-name' to 'new-name'"));

        // Verify old tag is gone, new tag exists
        let tags = store.load_tags(Scope::Project).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "new-name");
    }

    #[test]
    fn test_rename_tag_not_found() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result = rename_tag(&mut store, Scope::Project, "nonexistent", "new-name");
        assert!(result.is_err());
    }

    #[test]
    fn test_rename_tag_to_existing() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create_tag(&mut store, Scope::Project, "tag-a").unwrap();
        create_tag(&mut store, Scope::Project, "tag-b").unwrap();

        let result = rename_tag(&mut store, Scope::Project, "tag-a", "tag-b");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    #[test]
    fn test_rename_tag_invalid_new_name() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create_tag(&mut store, Scope::Project, "valid").unwrap();

        let result = rename_tag(&mut store, Scope::Project, "valid", "-invalid");
        assert!(result.is_err());
    }

    #[test]
    fn test_rename_tag_updates_pads() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create a tag and a pad with that tag
        create_tag(&mut store, Scope::Project, "old-tag").unwrap();
        create::run(
            &mut store,
            Scope::Project,
            "Test Pad".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        // Manually add tag to pad
        let mut pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        pads[0].metadata.tags.push("old-tag".to_string());
        store
            .save_pad(&pads[0], Scope::Project, Bucket::Active)
            .unwrap();

        // Rename the tag
        let result = rename_tag(&mut store, Scope::Project, "old-tag", "new-tag").unwrap();
        assert!(result.messages[1].content.contains("Updated 1 pad"));

        // Verify tag is updated in pad
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads[0].metadata.tags, vec!["new-tag"]);
    }

    #[test]
    fn test_list_tags_shows_count() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create_tag(&mut store, Scope::Project, "work").unwrap();
        create_tag(&mut store, Scope::Project, "rust").unwrap();

        let result = list_tags(&store, Scope::Project).unwrap();
        assert!(result.messages[0].content.contains("2 tags defined"));
    }

    #[test]
    fn test_ensure_tag_creates_new() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        ensure_tag(&mut store, Scope::Project, "work").unwrap();

        let tags = store.load_tags(Scope::Project).unwrap();
        assert_eq!(tags.len(), 1);
        assert_eq!(tags[0].name, "work");
    }

    #[test]
    fn test_ensure_tag_idempotent() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        ensure_tag(&mut store, Scope::Project, "work").unwrap();
        ensure_tag(&mut store, Scope::Project, "work").unwrap();

        let tags = store.load_tags(Scope::Project).unwrap();
        assert_eq!(tags.len(), 1);
    }

    #[test]
    fn test_ensure_tag_rejects_invalid() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let result = ensure_tag(&mut store, Scope::Project, "-invalid");
        assert!(result.is_err());
    }
}
