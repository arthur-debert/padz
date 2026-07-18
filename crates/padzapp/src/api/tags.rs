//! Tag registry CRUD and per-pad tagging.
//!
//! The facade preserves selector parsing while returning dedicated catalog and
//! mutation outcomes. It does not turn those facts into presentation messages.

use crate::commands;
use crate::commands::tagging::TaggingResult;
use crate::commands::tags::{TagCatalogOutcome, TagRegistryOutcome};
use crate::error::Result;
use crate::model::Scope;
use crate::store::DataStore;

use super::selectors::parse_selectors;
use super::PadzApi;

impl<S: DataStore> PadzApi<S> {
    /// List all tags in registry order, including an explicit empty state.
    pub fn list_tags(&self, scope: Scope) -> Result<TagCatalogOutcome> {
        commands::tags::list_tags(&self.store, scope)
    }

    /// List the unique tags for specific pads in lexical order.
    pub fn list_pad_tags<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<TagCatalogOutcome> {
        let selectors = parse_selectors(indexes)?;
        commands::tags::list_pad_tags(&self.store, scope, &selectors)
    }

    /// Create a new tag and return its semantic registry action.
    pub fn create_tag(&mut self, scope: Scope, name: &str) -> Result<TagRegistryOutcome> {
        commands::tags::create_tag(&mut self.store, scope, name)
    }

    /// Delete a tag and report how many pads the cascade changed.
    pub fn delete_tag(&mut self, scope: Scope, name: &str) -> Result<TagRegistryOutcome> {
        commands::tags::delete_tag(&mut self.store, scope, name)
    }

    /// Rename a tag and report how many pads were updated.
    pub fn rename_tag(
        &mut self,
        scope: Scope,
        old_name: &str,
        new_name: &str,
    ) -> Result<TagRegistryOutcome> {
        commands::tags::rename_tag(&mut self.store, scope, old_name, new_name)
    }

    /// Add requested tags and distinguish changed pads from an all-present no-op.
    pub fn add_tags_to_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        tags: &[String],
    ) -> Result<TaggingResult> {
        let selectors = parse_selectors(indexes)?;
        commands::tagging::add_tags(&mut self.store, scope, &selectors, tags)
    }

    /// Remove requested tags and distinguish changed pads from a none-present no-op.
    pub fn remove_tags_from_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        tags: &[String],
    ) -> Result<TaggingResult> {
        let selectors = parse_selectors(indexes)?;
        commands::tagging::remove_tags(&mut self.store, scope, &selectors, tags)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::test_support::make_api;
    use crate::commands::tagging::TaggingOutcome;
    use crate::commands::tags::{TagCatalogOutcome, TagRegistryOutcome};
    use crate::model::Scope;

    #[test]
    fn test_api_list_tags() {
        let mut api = make_api();
        api.create_tag(Scope::Project, "work").unwrap();

        let result = api.list_tags(Scope::Project).unwrap();

        assert_eq!(
            result,
            TagCatalogOutcome::Listed {
                tags: vec!["work".into()]
            }
        );
    }

    #[test]
    fn test_api_create_tag() {
        let mut api = make_api();

        let result = api.create_tag(Scope::Project, "rust").unwrap();

        assert_eq!(
            result,
            TagRegistryOutcome::Created {
                name: "rust".into(),
                affected_pads: 0,
            }
        );
    }

    #[test]
    fn test_api_delete_tag() {
        let mut api = make_api();
        api.create_tag(Scope::Project, "work").unwrap();

        let result = api.delete_tag(Scope::Project, "work").unwrap();

        assert_eq!(
            result,
            TagRegistryOutcome::Deleted {
                name: "work".into(),
                affected_pads: 0,
            }
        );
    }

    #[test]
    fn test_api_rename_tag() {
        let mut api = make_api();
        api.create_tag(Scope::Project, "old-name").unwrap();

        let result = api
            .rename_tag(Scope::Project, "old-name", "new-name")
            .unwrap();

        assert_eq!(
            result,
            TagRegistryOutcome::Renamed {
                old_name: "old-name".into(),
                new_name: "new-name".into(),
                affected_pads: 0,
            }
        );
    }

    #[test]
    fn test_api_add_tags_to_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.create_tag(Scope::Project, "work").unwrap();

        let result = api
            .add_tags_to_pads(Scope::Project, &["1"], &["work".to_string()])
            .unwrap();

        assert_eq!(
            result.outcome,
            TaggingOutcome::Assigned {
                requested_tags: vec!["work".into()],
                modified_pads: 1,
            }
        );
        assert!(result.affected_pads[0]
            .pad
            .metadata
            .tags
            .contains(&"work".to_string()));
    }

    #[test]
    fn test_api_remove_tags_from_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.create_tag(Scope::Project, "work").unwrap();
        api.add_tags_to_pads(Scope::Project, &["1"], &["work".to_string()])
            .unwrap();

        let result = api
            .remove_tags_from_pads(Scope::Project, &["1"], &["work".to_string()])
            .unwrap();

        assert_eq!(
            result.outcome,
            TaggingOutcome::Removed {
                requested_tags: vec!["work".into()],
                modified_pads: 1,
            }
        );
        assert!(result.affected_pads[0].pad.metadata.tags.is_empty());
    }
}
