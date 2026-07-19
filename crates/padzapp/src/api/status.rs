//! Pad status (pin, todo status) and hierarchy moves.

use crate::commands;
use crate::error::{PadzError, Result};
use crate::index::parse_index_or_range;
use crate::model::Scope;
use crate::store::DataStore;

use super::selectors::parse_selectors;
use super::PadzApi;

impl<S: DataStore> PadzApi<S> {
    pub fn pin_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::pinning::pin(&mut self.store, scope, &selectors)
    }

    pub fn unpin_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::pinning::unpin(&mut self.store, scope, &selectors)
    }

    /// Marks pads as Done, reporting already-done selectors as semantic no-ops.
    pub fn complete_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::status::complete(&mut self.store, scope, &selectors)
    }

    /// Reopens pads, reporting already-planned selectors as semantic no-ops.
    pub fn reopen_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        commands::status::reopen(&mut self.store, scope, &selectors)
    }

    /// Moves pads under a parent (or root), reporting same-parent moves as
    /// semantic no-ops with their canonical display paths.
    pub fn move_pads<I: AsRef<str>>(
        &mut self,
        scope: Scope,
        indexes: &[I],
        to_parent: Option<&str>,
    ) -> Result<commands::CmdResult> {
        let selectors = parse_selectors(indexes)?;
        let parent_selector = if let Some(p) = to_parent {
            if p.trim().is_empty() {
                None // Empty string means move to root
            } else {
                Some(parse_index_or_range(p).map_err(PadzError::Api)?)
            }
        } else {
            None
        };
        commands::move_pads::run(&mut self.store, scope, &selectors, parent_selector.as_ref())
    }

    /// Propagate todo status changes upward from a child's parent.
    ///
    /// Called after create/delete/status-change operations to keep ancestor
    /// statuses consistent. Separated from `create_pad` because propagation
    /// triggers reconciliation (via `list_pads`), which garbage-collects empty
    /// files — a problem when the pad hasn't been filled yet (editor flow).
    pub fn propagate_status(&mut self, scope: Scope, parent_id: Option<uuid::Uuid>) -> Result<()> {
        crate::todos::propagate_status_change(&mut self.store, scope, parent_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::api::test_support::make_api;
    use crate::api::PadFilter;
    use crate::model::Scope;

    #[test]
    fn test_api_pin_unpin_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Test".into(), "".into(), None)
            .unwrap();

        let result = api.pin_pads(Scope::Project, &["1"]).unwrap();
        assert_eq!(result.affected_pads.len(), 1);
        assert!(result.affected_pads[0].pad.metadata.is_pinned);

        let result = api.unpin_pads(Scope::Project, &["p1"]).unwrap();
        assert_eq!(result.affected_pads.len(), 1);
        assert!(!result.affected_pads[0].pad.metadata.is_pinned);
    }

    #[test]
    fn test_api_move_pads() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "A".into(), "".into(), None)
            .unwrap();

        std::thread::sleep(std::time::Duration::from_millis(10));

        api.create_pad(Scope::Project, "B".into(), "".into(), None)
            .unwrap();

        let result = api.move_pads(Scope::Project, &["1"], Some("2")).unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "B");

        let pads = api
            .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
            .unwrap()
            .listed_pads;
        let pad_a = pads.iter().find(|p| p.pad.metadata.title == "A").unwrap();
        assert_eq!(pad_a.children.len(), 1);
        assert_eq!(pad_a.children[0].pad.metadata.title, "B");
    }

    #[test]
    fn test_api_move_pads_to_root() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "Parent".into(), "".into(), None)
            .unwrap();
        api.create_pad(Scope::Project, "Child".into(), "".into(), Some("1"))
            .unwrap();

        let result = api.move_pads(Scope::Project, &["1.1"], None).unwrap();
        assert_eq!(result.affected_pads.len(), 1);

        let pads = api
            .get_pads(Scope::Project, PadFilter::default(), &[] as &[String])
            .unwrap()
            .listed_pads;
        let child = pads
            .iter()
            .find(|p| p.pad.metadata.title == "Child")
            .unwrap();
        assert!(child.pad.metadata.parent_id.is_none());
    }

    #[test]
    fn test_api_move_pads_cycle_error() {
        let mut api = make_api();
        api.create_pad(Scope::Project, "A".into(), "".into(), None)
            .unwrap();

        let result = api.move_pads(Scope::Project, &["1"], Some("1"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("into itself"));
    }
}
