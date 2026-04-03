use crate::commands::CmdResult;
use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad};
use crate::model::{Pad, Scope};
use crate::store::{Bucket, DataStore};

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    title: String,
    content: String,
    parent_selector: Option<crate::index::PadSelector>,
) -> Result<CmdResult> {
    let mut pad = Pad::new(title, content);

    if let Some(selector) = parent_selector {
        // Resolve parent
        let resolved = super::helpers::resolve_selectors(store, scope, &[selector], false)?;

        match resolved.len() {
            0 => {
                return Err(crate::error::PadzError::Api(
                    "Parent pad not found".to_string(),
                ))
            }
            1 => {
                let (_, parent_id) = resolved[0];
                pad.metadata.parent_id = Some(parent_id);
            }
            _ => {
                return Err(crate::error::PadzError::Api(
                    "Parent selector is ambiguous/multiple matches".to_string(),
                ))
            }
        }
    }

    store.save_pad(&pad, scope, Bucket::Active)?;

    // NOTE: We intentionally do NOT call propagate_status_change here.
    // Propagation triggers list_pads → reconciliation, which garbage-collects
    // empty content files. In the editor flow, the pad starts with empty content
    // (user hasn't typed yet), so reconciliation would delete the file AND its
    // index entry — destroying the parent_id. The caller is responsible for
    // calling propagate_status after the pad has real content.

    // Get the path for the created pad (for editor integration)
    let pad_path = store.get_pad_path(&pad.metadata.id, scope, Bucket::Active)?;

    let mut result = CmdResult::default();
    // New pad is always the newest, so it gets index 1
    let display_pad = DisplayPad {
        pad: pad.clone(),
        index: DisplayIndex::Regular(1),
        matches: None,
        children: Vec::new(),
    };
    result.affected_pads.push(display_pad);
    result.pad_paths.push(pad_path);
    // Note: No success message - CLI layer handles unified rendering
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{DisplayIndex, PadSelector};
    use crate::store::backend::StorageBackend;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    #[test]
    fn creates_nested_pad() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        // Create parent
        run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();

        // Create child
        let parent_sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(parent_sel),
        )
        .unwrap();

        // Check relationship
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let child = pads.iter().find(|p| p.metadata.title == "Child").unwrap();
        let parent = pads.iter().find(|p| p.metadata.title == "Parent").unwrap();

        assert_eq!(child.metadata.parent_id, Some(parent.metadata.id));
    }

    #[test]
    fn parent_not_found_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Try to create with non-existent parent index
        let result = run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(99)])),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn ambiguous_parent_selector_returns_error() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create two pads with similar titles
        run(
            &mut store,
            Scope::Project,
            "Meeting Notes Monday".into(),
            "".into(),
            None,
        )
        .unwrap();
        run(
            &mut store,
            Scope::Project,
            "Meeting Notes Tuesday".into(),
            "".into(),
            None,
        )
        .unwrap();

        // Try to create with ambiguous title search as parent
        let result = run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Title("Meeting".to_string())),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("multiple"));
    }

    #[test]
    fn create_returns_affected_pad_with_index_1() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        let result = run(
            &mut store,
            Scope::Project,
            "New Pad".into(),
            "Content".into(),
            None,
        )
        .unwrap();

        // Should have one affected pad
        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "New Pad");
        assert!(matches!(
            result.affected_pads[0].index,
            DisplayIndex::Regular(1)
        ));

        // No messages - CLI handles unified rendering
        assert!(result.messages.is_empty());
    }

    #[test]
    fn create_with_content_normalizes_title_in_content() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        let result = run(
            &mut store,
            Scope::Project,
            "My Title".into(),
            "Body text".into(),
            None,
        )
        .unwrap();

        let pad = &result.affected_pads[0].pad;
        // Title should be extracted and content should include title
        assert_eq!(pad.metadata.title, "My Title");
        assert!(pad.content.starts_with("My Title"));
    }

    #[test]
    fn create_root_pad_has_no_parent() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        run(&mut store, Scope::Project, "Root".into(), "".into(), None).unwrap();

        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        assert_eq!(pads.len(), 1);
        assert!(pads[0].metadata.parent_id.is_none());
    }

    /// Regression test: creating a nested pad with empty content must not trigger
    /// reconciliation (which would delete the empty file and lose parent_id).
    ///
    /// Before the fix, create::run called propagate_status_change which called
    /// list_pads → reconcile. Reconciliation garbage-collected the empty content
    /// file AND its index entry (with parent_id). Now create::run does NOT call
    /// propagation — the caller handles it after the pad has real content.
    #[test]
    fn nested_empty_content_no_propagation_during_create() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create parent
        run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();

        // Create child with EMPTY content — simulates `padz create -i 1 -e` (editor flow)
        let parent_sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        let result = run(
            &mut store,
            Scope::Project,
            "".into(),
            "".into(),
            Some(parent_sel),
        )
        .unwrap();
        let child_id = result.affected_pads[0].pad.metadata.id;
        let parent_id = result.affected_pads[0].pad.metadata.parent_id;

        // The child was created with parent_id set
        assert!(parent_id.is_some());

        // Crucially: get_pad must find the child (no reconciliation ran to delete it)
        let child = store
            .get_pad(&child_id, Scope::Project, Bucket::Active)
            .expect("Child pad must exist — create should not trigger reconciliation");
        assert_eq!(child.metadata.parent_id, parent_id);
    }

    /// Simulates the full editor flow: create empty → editor writes content →
    /// refresh → propagate. Parent_id must be preserved throughout.
    #[test]
    fn nested_editor_flow_preserves_parent_id() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        // Create parent
        run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();

        // Create child with empty content (editor hasn't filled it yet)
        let parent_sel = PadSelector::Path(vec![DisplayIndex::Regular(1)]);
        let result = run(
            &mut store,
            Scope::Project,
            "".into(),
            "".into(),
            Some(parent_sel),
        )
        .unwrap();
        let child_id = result.affected_pads[0].pad.metadata.id;
        let parent_id = result.affected_pads[0].pad.metadata.parent_id;
        assert!(parent_id.is_some());

        // Simulate editor writing content
        store
            .active_store_mut()
            .backend
            .write_content(&child_id, Scope::Project, "Editor Content")
            .unwrap();

        // Simulate refresh_pad: get_pad + update
        let mut pad = store
            .get_pad(&child_id, Scope::Project, Bucket::Active)
            .expect("Pad must exist after editor write");
        pad.update_from_raw("Editor Content");
        store
            .save_pad(&pad, Scope::Project, Bucket::Active)
            .unwrap();

        // NOW propagate (caller responsibility) — this triggers list_pads → reconcile
        crate::todos::propagate_status_change(&mut store, Scope::Project, parent_id).unwrap();

        // Verify parent_id survived the full cycle
        let final_pad = store
            .get_pad(&child_id, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(
            final_pad.metadata.parent_id, parent_id,
            "Parent ID must survive create → editor → refresh → propagate cycle"
        );
        assert_eq!(final_pad.metadata.title, "Editor Content");
    }

    #[test]
    fn create_returns_pad_path() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );

        let result = run(
            &mut store,
            Scope::Project,
            "Path Test".into(),
            "Body".into(),
            None,
        )
        .unwrap();

        // pad_paths should be populated for editor/clipboard integration
        assert_eq!(result.pad_paths.len(), 1);
        // Path should contain the pad's UUID
        let pad_id = result.affected_pads[0].pad.metadata.id.to_string();
        assert!(
            result.pad_paths[0].to_string_lossy().contains(&pad_id),
            "pad_path should contain the pad's UUID"
        );
    }
}
