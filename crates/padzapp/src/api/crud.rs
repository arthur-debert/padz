//! CRUD operations on pads.

use crate::commands;
use crate::error::{PadzError, Result};
use crate::index::parse_index_or_range;
use crate::model::Scope;
use crate::store::DataStore;

use super::selectors::{
    parse_selectors, parse_selectors_for_archived, parse_selectors_for_deleted,
};
use super::PadFilter;
use super::PadzApi;

impl<S: DataStore> PadzApi<S> {
    pub fn create_pad(
        &mut self,
        scope: Scope,
        title: String,
        content: String,
        parent: Option<&str>,
    ) -> Result<commands::CmdResult> {
        let parent_selector = if let Some(p) = parent {
            Some(parse_index_or_range(p).map_err(PadzError::Api)?)
        } else {
            None
        };
        commands::create::run(&mut self.store, scope, title, content, parent_selector)
    }

    pub fn get_pads<I: AsRef<str>>(
        &self,
        scope: Scope,
        filter: PadFilter,
        ids: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = if ids.is_empty() {
            vec![]
        } else {
            parse_selectors(ids)?
        };
        commands::get::run(&self.store, scope, filter, &selectors)
    }

    pub fn view_pads<I: AsRef<str>>(
        &self,
        scope: Scope,
        indexes: &[I],
        nesting: commands::NestingMode,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::view::run(&self.store, scope, &selectors, nesting)
    }

    pub fn delete_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::delete::run(&mut self.store, scope, &selectors)
    }

    /// Soft-deletes all active pads marked as Done (completed).
    pub fn delete_completed_pads(&mut self, scope: Scope) -> Result<commands::CmdResult> {
        commands::delete::run_completed(&mut self.store, scope)
    }

    pub fn update_pads(
        &mut self,
        scope: Scope,
        updates: &[commands::PadUpdate],
    ) -> Result<commands::CmdResult> {
        commands::update::run(&mut self.store, scope, updates)
    }

    /// Updates pads with raw content (e.g., from piped stdin).
    pub fn update_pads_from_content<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        raw_content: &str,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::update::run_from_content(&mut self.store, scope, &selectors, raw_content)
    }

    pub fn restore_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors_for_deleted(indexes)?;
        commands::restore::run(&mut self.store, scope, &selectors)
    }

    /// Permanently deletes pads.
    ///
    /// **Confirmation required**: The `confirmed` parameter must be `true` to proceed.
    /// Returns an empty outcome or the selected pads and completed deletion counts.
    pub fn purge_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        recursive: bool,
        confirmed: bool,
        include_done: bool,
    ) -> Result<commands::purge::PurgeOutcome> {
        let selectors = parse_selectors(indexes)?;
        commands::purge::run(
            &mut self.store,
            scope,
            &selectors,
            recursive,
            confirmed,
            include_done,
        )
    }

    pub fn archive_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::archive::run(&mut self.store, scope, &selectors)
    }

    pub fn unarchive_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors_for_archived(indexes)?;
        commands::unarchive::run(&mut self.store, scope, &selectors)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::test_support::make_api;
    use crate::api::{PadFilter, PadStatusFilter};
    use crate::commands::{NestingMode, PadUpdate};
    use crate::index::DisplayIndex;
    use crate::model::Scope;

    #[test]
    fn test_api_create_pad_simple() {
        let mut api = make_api();

        let result = api
            .create_pad(Scope::Project, "Test Title".into(), "Content".into(), None)
            .unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "Test Title");
    }

    #[test]
    fn test_api_create_pad_with_parent_string() {
        let mut api = make_api();

        api.create_pad(Scope::Project, "Parent".into(), "".into(), None)
            .unwrap();

        let result = api
            .create_pad(Scope::Project, "Child".into(), "".into(), Some("1"))
            .unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "Child");
        assert!(result.affected_pads[0].pad.metadata.parent_id.is_some());
    }

    #[test]
    fn test_api_get_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();

        let result = api
            .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
            .unwrap();

        assert_eq!(result.listed_pads.len(), 1);
    }

    #[test]
    fn test_api_view_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();

        let result = api
            .view_pads(Scope::Project, &["1"], NestingMode::Flat)
            .unwrap();

        assert_eq!(result.listed_pads.len(), 1);
        assert_eq!(result.listed_pads[0].pad.metadata.title, "Test");
    }

    #[test]
    fn test_api_delete_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();

        let result = api.delete_pads(Scope::Project, &["1"]).unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert!(matches!(
            result.affected_pads[0].index,
            DisplayIndex::Deleted(_)
        ));
    }

    #[test]
    fn test_api_update_pads() {
        let mut api = make_api();
        api.create_pad(
            Scope::Project,
            "Old Title".into(),
            "Old Content".into(),
            None,
        )
        .unwrap();

        let updates = vec![PadUpdate::new(
            DisplayIndex::Regular(1),
            "New Title".into(),
            "New Content".into(),
        )];
        let result = api.update_pads(Scope::Project, &updates).unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "New Title");
    }

    #[test]
    fn test_api_restore_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.delete_pads(Scope::Project, &["1"]).unwrap();

        let result = api.restore_pads(Scope::Project, &["1"]).unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert!(matches!(
            result.affected_pads[0].index,
            DisplayIndex::Regular(_)
        ));
    }

    #[test]
    fn test_api_purge_pads_confirmed() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.delete_pads(Scope::Project, &["1"]).unwrap();

        let result = api.purge_pads(Scope::Project, &["d1"], false, true, false);
        assert!(result.is_ok());

        let list = api
            .get_pads(
                Scope::Project,
                PadFilter {
                    status: PadStatusFilter::All,
                    search_term: None,
                    todo_status: None,
                    tags: None,
                },
                &[] as &[String],
            )
            .unwrap();
        assert_eq!(list.listed_pads.len(), 0);
    }

    #[test]
    fn test_api_purge_pads_not_confirmed() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();
        api.delete_pads(Scope::Project, &["1"]).unwrap();

        let result = api.purge_pads(Scope::Project, &["d1"], false, false, false);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("Aborted"));

        let list = api
            .get_pads(
                Scope::Project,
                PadFilter {
                    status: PadStatusFilter::Deleted,
                    search_term: None,
                    todo_status: None,
                    tags: None,
                },
                &[] as &[String],
            )
            .unwrap();
        assert_eq!(list.listed_pads.len(), 1);
    }
}
