//! Tag registry CRUD and per-pad tagging.

use crate::commands;
use crate::error::Result;
use crate::model::Scope;
use crate::store::DataStore;

use super::selectors::parse_selectors;
use super::PadzApi;

impl<S: DataStore> PadzApi<S> {
    /// List all tags in the registry.
    pub fn list_tags(&self, scope: Scope) -> Result<commands::CmdResult> {
        commands::tags::list_tags(&self.store, scope)
    }

    /// List tags for specific pads.
    pub fn list_pad_tags<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::tags::list_pad_tags(&self.store, scope, &selectors)
    }

    /// Create a new tag in the registry.
    pub fn create_tag(&mut self, scope: Scope, name: &str) -> Result<commands::CmdResult> {
        commands::tags::create_tag(&mut self.store, scope, name)
    }

    /// Delete a tag from the registry (cascades to remove from all pads).
    pub fn delete_tag(&mut self, scope: Scope, name: &str) -> Result<commands::CmdResult> {
        commands::tags::delete_tag(&mut self.store, scope, name)
    }

    /// Rename a tag in the registry (updates all pads).
    pub fn rename_tag(
        &mut self,
        scope: Scope,
        old_name: &str,
        new_name: &str,
    ) -> Result<commands::CmdResult> {
        commands::tags::rename_tag(&mut self.store, scope, old_name, new_name)
    }

    /// Add tags to pads.
    pub fn add_tags_to_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        tags: &[String],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::tagging::add_tags(&mut self.store, scope, &selectors, tags)
    }

    /// Remove tags from pads.
    pub fn remove_tags_from_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        tags: &[String],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::tagging::remove_tags(&mut self.store, scope, &selectors, tags)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::test_support::make_api;
    use crate::model::Scope;

    #[test]
    fn test_api_list_tags() {
        let mut api = make_api();
        api.create_tag(Scope::Project, "work").unwrap();

        let result = api.list_tags(Scope::Project).unwrap();

        assert!(result.messages.iter().any(|m| m.content.contains("work")));
    }

    #[test]
    fn test_api_create_tag() {
        let mut api = make_api();

        let result = api.create_tag(Scope::Project, "rust").unwrap();

        assert!(result.messages[0].content.contains("Created tag 'rust'"));
    }

    #[test]
    fn test_api_delete_tag() {
        let mut api = make_api();
        api.create_tag(Scope::Project, "work").unwrap();

        let result = api.delete_tag(Scope::Project, "work").unwrap();

        assert!(result.messages[0].content.contains("Deleted tag 'work'"));
    }

    #[test]
    fn test_api_rename_tag() {
        let mut api = make_api();
        api.create_tag(Scope::Project, "old-name").unwrap();

        let result = api
            .rename_tag(Scope::Project, "old-name", "new-name")
            .unwrap();

        assert!(result.messages[0]
            .content
            .contains("Renamed tag 'old-name' to 'new-name'"));
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

        assert!(result.messages[0].content.contains("Added tag"));
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

        assert!(result.messages[0].content.contains("Removed tag"));
        assert!(result.affected_pads[0].pad.metadata.tags.is_empty());
    }
}
