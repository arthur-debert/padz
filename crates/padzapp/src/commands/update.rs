use crate::commands::{CmdMessage, CmdResult, PadUpdate};
use crate::error::Result;
use crate::index::{DisplayPad, PadSelector};
use crate::model::Scope;
use crate::store::DataStore;
use chrono::Utc;

use super::helpers::resolve_selectors;

pub fn run<S: DataStore>(store: &mut S, scope: Scope, updates: &[PadUpdate]) -> Result<CmdResult> {
    if updates.is_empty() {
        return Ok(CmdResult::default());
    }

    let selectors: Vec<_> = updates
        .iter()
        .map(|u| PadSelector::Path(vec![u.index.clone()]))
        .collect();
    let resolved = resolve_selectors(store, scope, &selectors, false)?;
    let mut result = CmdResult::default();

    for ((display_index, uuid), update) in resolved.into_iter().zip(updates.iter()) {
        let mut pad = store.get_pad(&uuid, scope)?;
        // We accept updates from editor which splits title and body.
        // We must re-normalize to get the correct full content.
        let (_, normalized_content) =
            crate::model::normalize_pad_content(&update.title, &update.content);

        pad.metadata.title = update.title.clone();
        pad.metadata.updated_at = Utc::now();
        pad.content = normalized_content;
        store.save_pad(&pad, scope)?;

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
}
