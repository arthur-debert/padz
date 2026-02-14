use crate::commands::{CmdMessage, CmdResult, DisplayPad};
use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, PadSelector};
use crate::model::Scope;
use crate::store::{Bucket, DataStore};
use chrono::Utc;
use uuid::Uuid;

use super::helpers::{fmt_path, resolve_selectors};

pub fn run<S: DataStore>(
    store: &mut S,
    scope: Scope,
    selectors: &[PadSelector],
    destination_selector: Option<&PadSelector>,
) -> Result<CmdResult> {
    // 1. Resolve source pads
    let resolved_sources = resolve_selectors(store, scope, selectors, false)?;

    if resolved_sources.is_empty() {
        return Ok(CmdResult::default());
    }

    // 2. Resolve destination (if any)
    let destination_data = if let Some(dest_sel) = destination_selector {
        let resolved_dests =
            resolve_selectors(store, scope, std::slice::from_ref(dest_sel), false)?;
        if resolved_dests.is_empty() {
            return Err(PadzError::Api(format!(
                "Destination not found: {:?}",
                dest_sel
            )));
        }
        if resolved_dests.len() > 1 {
            return Err(PadzError::Api(
                "Destination selector must resolve to a single pad".to_string(),
            ));
        }
        let (_dest_idx, dest_id) = resolved_dests.into_iter().next().unwrap();

        // Load destination to get its title for the message later
        let dest_pad = store.get_pad(&dest_id, scope, Bucket::Active)?;
        Some((dest_id, dest_pad.metadata.title))
    } else {
        None
    };

    let dest_uuid = destination_data.map(|(uuid, _)| uuid);

    let mut result = CmdResult::default();

    // We need to keep track of processed IDs to avoid duplicates if the user selected ranges overlapping
    let mut processed_ids = std::collections::HashSet::new();

    for (display_index, source_uuid) in resolved_sources {
        if !processed_ids.insert(source_uuid) {
            continue;
        }

        // 3. Validation

        // 3a. Cannot move to self
        if Some(source_uuid) == dest_uuid {
            return Err(PadzError::Api(format!(
                "Cannot move pad '{}' into itself",
                fmt_path(&display_index)
            )));
        }

        // 3b. Cycle detection: Cannot move to a descendant
        if let Some(target_id) = dest_uuid {
            if is_descendant_of(store, scope, target_id, source_uuid)? {
                return Err(PadzError::Api(format!(
                    "Cannot move pad '{}' into its own descendant",
                    fmt_path(&display_index)
                )));
            }
        }

        // 4. Update
        let mut pad = store.get_pad(&source_uuid, scope, Bucket::Active)?;

        // Skip if already there
        if pad.metadata.parent_id == dest_uuid {
            result.add_message(CmdMessage::info(format!(
                "Pad '{}' is already at destination",
                fmt_path(&display_index)
            )));
            continue;
        }

        pad.metadata.parent_id = dest_uuid;
        pad.metadata.updated_at = Utc::now();

        store.save_pad(&pad, scope, Bucket::Active)?;

        // Note: No success message - CLI handles unified rendering

        // Note: The index in result will be the *old* index because we haven't re-indexed the world.
        // But for the purpose of "affected pads", we return the pad state.
        result.affected_pads.push(DisplayPad {
            pad,
            index: display_index
                .last()
                .cloned()
                .unwrap_or(DisplayIndex::Regular(0)), // Best effort local index
            matches: None,
            children: Vec::new(),
        });
    }

    Ok(result)
}

/// Checks if `child_id` is a descendant of `potential_ancestor_id` (recursive check up the tree).
fn is_descendant_of<S: DataStore>(
    store: &S,
    scope: Scope,
    child_id: Uuid,
    potential_ancestor_id: Uuid,
) -> Result<bool> {
    let mut current_id = child_id;

    // Safety break against infinite loops in corrupt trees (depth limit)
    let mut depth = 0;
    const MAX_DEPTH: u32 = 1000;

    while depth < MAX_DEPTH {
        // Optimization: In a real DB we'd use a recursive query or materialized path.
        // Here we walk up.
        let pad = store.get_pad(&current_id, scope, Bucket::Active)?;

        if let Some(parent_id) = pad.metadata.parent_id {
            if parent_id == potential_ancestor_id {
                return Ok(true);
            }
            current_id = parent_id;
            depth += 1;
        } else {
            // Reached root
            return Ok(false);
        }
    }

    // If we hit max depth, assume cycle or deep tree, but conservatively safest to default false if we trust tree integrity?
    // Actually if we suspect a cycle in existing data (which shouldn't happen), we might loop forever without MAX_DEPTH.
    // If we hit MAX_DEPTH, we can warn and stop.
    // For now assuming 1000 is deep enough.
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::create;
    use crate::index::DisplayIndex;
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    fn setup_store() -> (BucketedStore<MemBackend>, Uuid, Uuid) {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let root_res = create::run(
            &mut store,
            Scope::Project,
            "Root".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();
        let root_id = root_res.affected_pads[0].pad.metadata.id;

        let child_res = create::run(
            &mut store,
            Scope::Project,
            "Child".into(),
            "".into(),
            Some(PadSelector::Path(vec![DisplayIndex::Regular(1)])),
            Vec::new(),
        )
        .unwrap();
        let child_id = child_res.affected_pads[0].pad.metadata.id; // Corrected: this is the child's ID not root's

        (store, root_id, child_id)
    }

    #[test]
    fn test_move_child_to_root() {
        let (mut store, _root_id, child_id) = setup_store();

        // Verify initial state: child has parent
        let child = store
            .get_pad(&child_id, Scope::Project, Bucket::Active)
            .unwrap();
        assert!(child.metadata.parent_id.is_some());

        // Move to root (destination = None)
        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])],
            None,
        )
        .unwrap();

        // Verify: parent_id is None
        let child_after = store
            .get_pad(&child_id, Scope::Project, Bucket::Active)
            .unwrap();
        assert!(child_after.metadata.parent_id.is_none());
    }

    #[test]
    fn test_move_root_to_another_pad() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        // Create Pad A
        create::run(
            &mut store,
            Scope::Project,
            "A".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();
        // Create Pad B (Index 1 - Newest)
        create::run(
            &mut store,
            Scope::Project,
            "B".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        // Move B (1) into A (2)
        run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            Some(&PadSelector::Path(vec![DisplayIndex::Regular(2)])),
        )
        .unwrap();

        // Verify B's parent is A
        let pads = store.list_pads(Scope::Project, Bucket::Active).unwrap();
        let pad_a = pads.iter().find(|p| p.metadata.title == "A").unwrap();
        let pad_b = pads.iter().find(|p| p.metadata.title == "B").unwrap();

        assert_eq!(pad_b.metadata.parent_id, Some(pad_a.metadata.id));
    }

    #[test]
    fn test_prevent_move_to_self() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        create::run(
            &mut store,
            Scope::Project,
            "A".into(),
            "".into(),
            None,
            Vec::new(),
        )
        .unwrap();

        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])],
            Some(&PadSelector::Path(vec![DisplayIndex::Regular(1)])),
        );

        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("into itself"));
    }

    #[test]
    fn test_prevent_cycle_move_to_descendant() {
        let (mut store, _root_id, _child_id) = setup_store();
        // Root (1) -> Child (1.1)
        // Try to move Root (1) into Child (1.1) via selectors
        // Root is "1", Child is "1.1"

        let res = run(
            &mut store,
            Scope::Project,
            &[PadSelector::Path(vec![DisplayIndex::Regular(1)])], // Source: Root
            Some(&PadSelector::Path(vec![
                DisplayIndex::Regular(1),
                DisplayIndex::Regular(1),
            ])), // Dest: Child
        );

        assert!(res.is_err());
        // Depending on exact impl, fetching "1.1" might fail if it uses get_pad_at_path and relies on index which might shift?
        // But here the structure exists.
        assert!(res.unwrap_err().to_string().contains("descendant"));
    }
}
