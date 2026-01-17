use crate::commands::{CmdMessage, CmdResult, PadUpdate};
use crate::error::{PadzError, Result};
use crate::index::{DisplayPad, PadSelector};
use crate::model::{parse_pad_content, Scope};
use crate::store::DataStore;
use chrono::Utc;

use super::helpers::resolve_selectors;

pub fn run<S: DataStore>(store: &mut S, scope: Scope, updates: &[PadUpdate]) -> Result<CmdResult> {
    if updates.is_empty() {
        return Ok(CmdResult::default());
    }

    let selectors: Vec<_> = updates
        .iter()
        .map(|u| {
            if let Some(path) = &u.path {
                PadSelector::Path(path.clone())
            } else {
                PadSelector::Path(vec![u.index.clone()])
            }
        })
        .collect();
    let resolved = resolve_selectors(store, scope, &selectors, false)?;
    let mut result = CmdResult::default();

    for ((display_index, uuid), update) in resolved.into_iter().zip(updates.iter()) {
        let mut pad = store.get_pad(&uuid, scope)?;
        // We accept updates from editor which splits title and body.
        // We must re-normalize to get the correct full content.
        let (_, normalized_content) =
            crate::model::normalize_pad_content(&update.title, &update.content);

        if let Some(status) = update.status {
            pad.metadata.status = status;
        }

        pad.metadata.title = update.title.clone();
        pad.metadata.updated_at = Utc::now();
        pad.content = normalized_content;

        let parent_id = pad.metadata.parent_id;
        store.save_pad(&pad, scope)?;

        // Propagate status change to parent
        crate::todos::propagate_status_change(store, scope, parent_id)?;

        // Fix: Use display_index directly as it's already formatted for display
        result.add_message(CmdMessage::success(format!(
            "Pad updated ({}): {}",
            super::helpers::fmt_path(&display_index),
            pad.metadata.title
        )));
        // Index doesn't change after update (use the last segment of the path)
        let local_index = display_index
            .last()
            .cloned()
            .unwrap_or(update.index.clone());
        result.affected_pads.push(DisplayPad {
            pad,
            index: local_index,
            matches: None,
            children: Vec::new(),
        });
    }

    Ok(result)
}

/// Updates pads from raw content (e.g., from piped stdin).
///
/// This function takes raw content, parses it to extract title and body,
/// then applies the same content to all matching pads. Useful for pipe workflows
/// like: `cat content.md | padz open 1`
pub fn run_from_content<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    raw_content: &str,
) -> Result<CmdResult> {
    // Parse raw content to get title and normalized content
    let (title, full_content) = parse_pad_content(raw_content)
        .ok_or_else(|| PadzError::Api("Piped content is empty or invalid".to_string()))?;

    // Resolve selectors to UUIDs
    let resolved = resolve_selectors(store, scope, selectors, false)?;

    let mut result = CmdResult::default();

    for (display_index, uuid) in resolved {
        let mut pad = store.get_pad(&uuid, scope)?;

        // Update the pad with the new content
        pad.metadata.title = title.clone();
        pad.metadata.updated_at = Utc::now();
        pad.content = full_content.clone();

        let parent_id = pad.metadata.parent_id;
        store.save_pad(&pad, scope)?;

        // Propagate status change to parent
        crate::todos::propagate_status_change(store, scope, parent_id)?;

        result.add_message(CmdMessage::success(format!(
            "Updated ({}): {}",
            super::helpers::fmt_path(&display_index),
            pad.metadata.title
        )));

        let local_index = display_index
            .last()
            .cloned()
            .unwrap_or(display_index[0].clone());
        result.affected_pads.push(DisplayPad {
            pad,
            index: local_index,
            matches: None,
            children: Vec::new(),
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{create, get, view};
    use crate::index::{DisplayIndex, PadSelector};
    use crate::model::Scope;
    use crate::store::memory::InMemoryStore;

    #[test]
    fn updates_pad_content() {
        let mut store = InMemoryStore::new();
        create::run(
            &mut store,
            Scope::Project,
            "Title".into(),
            "Old".into(),
            None,
        )
        .unwrap();
        let update = PadUpdate::new(DisplayIndex::Regular(1), "Title".into(), "New".into());
        run(&mut store, Scope::Project, &[update]).unwrap();

        let pads = view::run(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap()
        .listed_pads;
        assert_eq!(pads[0].pad.content, "Title\n\nNew");
    }
    #[test]
    fn update_empty_batch_does_nothing() {
        let mut store = InMemoryStore::new();
        let result = run(&mut store, Scope::Project, &[]).unwrap();
        assert!(result.messages.is_empty());
        assert!(result.affected_pads.is_empty());
    }

    #[test]
    fn update_renames_title() {
        let mut store = InMemoryStore::new();
        create::run(
            &mut store,
            Scope::Project,
            "Old Title".into(),
            "Content".into(),
            None,
        )
        .unwrap();

        let update = PadUpdate::new(
            DisplayIndex::Regular(1),
            "New Title".into(),
            "Content".into(),
        );
        run(&mut store, Scope::Project, &[update]).unwrap();

        let pads = view::run(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap()
        .listed_pads;

        assert_eq!(pads[0].pad.metadata.title, "New Title");
        assert_eq!(pads[0].pad.content, "New Title\n\nContent");
    }

    #[test]
    fn update_batch_multiple_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "B".into(), "".into(), None).unwrap();

        let updates = vec![
            PadUpdate::new(
                DisplayIndex::Regular(1),
                "A Updated".into(),
                "Content A".into(),
            ),
            PadUpdate::new(
                DisplayIndex::Regular(2),
                "B Updated".into(),
                "Content B".into(),
            ),
        ];

        let res = run(&mut store, Scope::Project, &updates).unwrap();

        // Check results
        assert_eq!(res.messages.len(), 2);
        assert!(res.messages.iter().any(|m| m.content.contains("A Updated")));
        assert!(res.messages.iter().any(|m| m.content.contains("B Updated")));

        // Check store state
        let pads = get::run(&store, Scope::Project, get::PadFilter::default())
            .unwrap()
            .listed_pads;
        let pad_a = pads
            .iter()
            .find(|p| p.pad.metadata.title == "A Updated")
            .unwrap();
        let pad_b = pads
            .iter()
            .find(|p| p.pad.metadata.title == "B Updated")
            .unwrap();

        assert_eq!(pad_a.pad.content, "A Updated\n\nContent A");
        assert_eq!(pad_b.pad.content, "B Updated\n\nContent B");
    }

    #[test]
    fn update_normalizes_content_structure() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();

        // Update with content that needs normalization (title not in body)
        let update = PadUpdate::new(
            DisplayIndex::Regular(1),
            "New Title".into(),
            "Body content".into(),
        );
        run(&mut store, Scope::Project, &[update]).unwrap();

        let pads = view::run(
            &store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
        )
        .unwrap()
        .listed_pads;

        // Title in metadata is set from update
        assert_eq!(pads[0].pad.metadata.title, "New Title");
        // Content is normalized with title at start and blank separator
        assert_eq!(pads[0].pad.content, "New Title\n\nBody content");
    }

    #[test]
    fn update_updates_timestamp() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();

        let pads_before = store.list_pads(Scope::Project).unwrap();
        let original_updated_at = pads_before[0].metadata.updated_at;

        // Small delay to ensure timestamp differs
        std::thread::sleep(std::time::Duration::from_millis(10));

        let update = PadUpdate::new(
            DisplayIndex::Regular(1),
            "Title".into(),
            "New Content".into(),
        );
        run(&mut store, Scope::Project, &[update]).unwrap();

        let pads_after = store.list_pads(Scope::Project).unwrap();
        let new_updated_at = pads_after[0].metadata.updated_at;

        assert!(new_updated_at > original_updated_at);
    }

    #[test]
    fn update_returns_affected_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();

        let update = PadUpdate::new(
            DisplayIndex::Regular(1),
            "Updated Title".into(),
            "Content".into(),
        );
        let result = run(&mut store, Scope::Project, &[update]).unwrap();

        // Should have one affected pad
        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(result.affected_pads[0].pad.metadata.title, "Updated Title");
        assert!(matches!(
            result.affected_pads[0].index,
            DisplayIndex::Regular(1)
        ));
    }

    #[test]
    fn update_success_message_format() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();

        let update = PadUpdate::new(
            DisplayIndex::Regular(1),
            "Updated Title".into(),
            "Content".into(),
        );
        let result = run(&mut store, Scope::Project, &[update]).unwrap();

        assert_eq!(result.messages.len(), 1);
        assert!(result.messages[0].content.contains("Pad updated"));
        assert!(result.messages[0].content.contains("1")); // index
        assert!(result.messages[0].content.contains("Updated Title"));
    }

    #[test]
    fn update_nonexistent_pad_fails() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();

        let update = PadUpdate::new(DisplayIndex::Regular(99), "Title".into(), "Content".into());
        let result = run(&mut store, Scope::Project, &[update]);

        assert!(result.is_err());
    }

    #[test]
    fn update_preserves_other_metadata() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();

        let pads_before = store.list_pads(Scope::Project).unwrap();
        let original_created_at = pads_before[0].metadata.created_at;
        let original_id = pads_before[0].metadata.id;

        let update = PadUpdate::new(
            DisplayIndex::Regular(1),
            "New Title".into(),
            "Content".into(),
        );
        run(&mut store, Scope::Project, &[update]).unwrap();

        let pads_after = store.list_pads(Scope::Project).unwrap();
        // Created_at should NOT change
        assert_eq!(pads_after[0].metadata.created_at, original_created_at);
        // ID should NOT change
        assert_eq!(pads_after[0].metadata.id, original_id);
    }

    #[test]
    fn test_status_propagation_via_update() {
        let mut store = InMemoryStore::new();

        // 1. Create a parent (initially Planned)
        create::run(&mut store, Scope::Project, "Parent".into(), "".into(), None).unwrap();

        // 2. Create a child (initially Planned)
        create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        )
        .unwrap();

        // Parent should still be Planned
        let pads = store.list_pads(Scope::Project).unwrap();
        let parent = pads.iter().find(|p| p.metadata.title == "Parent").unwrap();
        assert_eq!(parent.metadata.status, crate::model::TodoStatus::Planned);

        // 3. Update Child (1.1) to Done via PadUpdate using path
        let update = PadUpdate::new(
            DisplayIndex::Regular(1), // Ignored when path is present
            "Child".into(),
            "".into(),
        )
        .with_path(vec![DisplayIndex::Regular(1), DisplayIndex::Regular(1)])
        .with_status(crate::model::TodoStatus::Done);

        run(&mut store, Scope::Project, &[update]).unwrap();

        // Verify Child is Done
        let pads_after = store.list_pads(Scope::Project).unwrap();
        let child = pads_after
            .iter()
            .find(|p| p.metadata.title == "Child")
            .unwrap();
        assert_eq!(child.metadata.status, crate::model::TodoStatus::Done);

        // Verify Parent propagated to Done (All children done -> parent done)
        let parent_after = pads_after
            .iter()
            .find(|p| p.metadata.title == "Parent")
            .unwrap();
        assert_eq!(parent_after.metadata.status, crate::model::TodoStatus::Done);
    }

    // --- run_from_content tests (piped content updates) ---

    #[test]
    fn run_from_content_updates_single_pad() {
        let mut store = InMemoryStore::new();
        create::run(
            &mut store,
            Scope::Project,
            "Original Title".into(),
            "Original body".into(),
            None,
        )
        .unwrap();

        let raw_content = "New Title From Pipe\n\nNew body content from piped input.";
        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];

        let result = run_from_content(&mut store, Scope::Project, &selectors, raw_content).unwrap();

        assert_eq!(result.affected_pads.len(), 1);
        assert_eq!(
            result.affected_pads[0].pad.metadata.title,
            "New Title From Pipe"
        );
        assert_eq!(
            result.affected_pads[0].pad.content,
            "New Title From Pipe\n\nNew body content from piped input."
        );
    }

    #[test]
    fn run_from_content_updates_multiple_pads() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Pad A".into(), "".into(), None).unwrap();
        create::run(&mut store, Scope::Project, "Pad B".into(), "".into(), None).unwrap();

        let raw_content = "Shared New Title\n\nShared content for all selected pads.";
        let selectors = vec![
            PadSelector::Path(vec![DisplayIndex::Regular(1)]),
            PadSelector::Path(vec![DisplayIndex::Regular(2)]),
        ];

        let result = run_from_content(&mut store, Scope::Project, &selectors, raw_content).unwrap();

        assert_eq!(result.affected_pads.len(), 2);
        for dp in &result.affected_pads {
            assert_eq!(dp.pad.metadata.title, "Shared New Title");
            assert_eq!(
                dp.pad.content,
                "Shared New Title\n\nShared content for all selected pads."
            );
        }
    }

    #[test]
    fn run_from_content_empty_content_fails() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let raw_content = "   \n\n   "; // Only whitespace
        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];

        let result = run_from_content(&mut store, Scope::Project, &selectors, raw_content);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty or invalid"));
    }

    #[test]
    fn run_from_content_preserves_metadata() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Title".into(), "".into(), None).unwrap();

        let pads_before = store.list_pads(Scope::Project).unwrap();
        let original_id = pads_before[0].metadata.id;
        let original_created_at = pads_before[0].metadata.created_at;

        std::thread::sleep(std::time::Duration::from_millis(10));

        let raw_content = "New Title\n\nNew content";
        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];
        run_from_content(&mut store, Scope::Project, &selectors, raw_content).unwrap();

        let pads_after = store.list_pads(Scope::Project).unwrap();
        // ID should NOT change
        assert_eq!(pads_after[0].metadata.id, original_id);
        // Created_at should NOT change
        assert_eq!(pads_after[0].metadata.created_at, original_created_at);
        // Updated_at SHOULD change
        assert!(pads_after[0].metadata.updated_at > original_created_at);
    }

    #[test]
    fn run_from_content_title_only() {
        let mut store = InMemoryStore::new();
        create::run(
            &mut store,
            Scope::Project,
            "Original".into(),
            "Original body".into(),
            None,
        )
        .unwrap();

        // Content with just a title, no body
        let raw_content = "Title Only";
        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(1)])];

        let result = run_from_content(&mut store, Scope::Project, &selectors, raw_content).unwrap();

        assert_eq!(result.affected_pads[0].pad.metadata.title, "Title Only");
        assert_eq!(result.affected_pads[0].pad.content, "Title Only");
    }

    #[test]
    fn run_from_content_nonexistent_pad_fails() {
        let mut store = InMemoryStore::new();
        create::run(&mut store, Scope::Project, "Test".into(), "".into(), None).unwrap();

        let raw_content = "New Content\n\nBody";
        let selectors = vec![PadSelector::Path(vec![DisplayIndex::Regular(99)])];

        let result = run_from_content(&mut store, Scope::Project, &selectors, raw_content);
        assert!(result.is_err());
    }
}
