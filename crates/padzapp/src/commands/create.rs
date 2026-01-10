use crate::commands::{CmdMessage, CmdResult};
use crate::error::Result;
use crate::index::{DisplayIndex, DisplayPad};
use crate::model::{Pad, Scope};
use crate::store::DataStore;

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

    store.save_pad(&pad, scope)?;

    // Propagate status change to parent (e.g. adding a "Planned" child might revert parent from "Done")
    crate::todos::propagate_status_change(store, scope, pad.metadata.parent_id)?;

    // Get the path for the created pad (for editor integration)
    let pad_path = store.get_pad_path(&pad.metadata.id, scope)?;

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
    result.add_message(CmdMessage::success(format!(
        "Pad created: {}",
        pad.metadata.title
    )));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{DisplayIndex, PadSelector};
    use crate::store::memory::InMemoryStore;

    #[test]
    fn creates_nested_pad() {
        let mut store = InMemoryStore::new();
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
        let pads = store.list_pads(Scope::Project).unwrap();
        let child = pads.iter().find(|p| p.metadata.title == "Child").unwrap();
        let parent = pads.iter().find(|p| p.metadata.title == "Parent").unwrap();

        assert_eq!(child.metadata.parent_id, Some(parent.metadata.id));
    }

    #[test]
    fn parent_not_found_returns_error() {
        let mut store = InMemoryStore::new();

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
        let mut store = InMemoryStore::new();

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
        let mut store = InMemoryStore::new();

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

        // Should have success message
        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].content.contains("Pad created"));
        assert!(result.messages[0].content.contains("New Pad"));
    }

    #[test]
    fn create_with_content_normalizes_title_in_content() {
        let mut store = InMemoryStore::new();

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
        let mut store = InMemoryStore::new();

        run(&mut store, Scope::Project, "Root".into(), "".into(), None).unwrap();

        let pads = store.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 1);
        assert!(pads[0].metadata.parent_id.is_none());
    }
}
