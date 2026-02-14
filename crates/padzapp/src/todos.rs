//! # Notes as Todos: Design and Implementation
//!
//! This module implements the "Notes as Todos" feature, allowing pads to have a status
//! (Planned, In Progress, Done) and defining how these statuses propagate in a hierarchy.
//!
//! ## Statuses
//!
//! A pad can have one of three statuses:
//! - [`TodoStatus::Planned`][]: Tests/tasks to be done.
//! - [`TodoStatus::InProgress`][]: Currently being worked on.
//! - [`TodoStatus::Done`][]: Completed.
//!
//! Statuses are stored in the [`crate::model::Metadata`] struct.
//!
//! ## Nested Status Propagation
//!
//! For nested pads, the parent status is generally a function of its children's statuses.
//! This relationship ensures that the parent reflects the aggregate state of its sub-tasks.
//!
//! ### Propagation Rules
//!
//! 1. **All Done -> Done**: If all children are [`TodoStatus::Done`], the parent becomes [`TodoStatus::Done`].
//! 2. **All Planned -> Planned**: If all children are [`TodoStatus::Planned`], the parent becomes [`TodoStatus::Planned`].
//! 3. **Mixed -> In Progress**: If children are in mixed states (e.g., some Done, some Planned, or any InProgress),
//!    the parent becomes [`TodoStatus::InProgress`].
//!    - Specifically: If not all are Done, but at least one is Done (or InProgress), the parent is InProgress.
//!
//! ### Manual Override vs. Automatic Updates
//!
//! Users can manually set the status of a parent pad.
//! - **Manual Override**: If a user explicitly sets a parent to "Done", it becomes "Done" immediately.
//!   This creates a potential inconsistency (Parent=Done, Children=Planned), which is *allowed* by design.
//!   Downward propagation (changing children to match parent) is **NOT** performed.
//! - **Automatic Reaction**: However, subsequent changes to children *will* trigger a re-calculation of the parent status.
//!   The "Manual Override" is ephemeral; it holds until the next event that triggers a recalculation.
//!
//! ### Implementation Logic
//!
//! The propagation logic is "Bottom-Up":
//! - When a pad is created, deleted, or its status changes:
//!   1. Identify its parent.
//!   2. If no parent, stop.
//!   3. Fetch all siblings of the pad (children of the parent).
//!   4. Calculate the derived status based on siblings.
//!   5. If derived status != parent's current status:
//!      - Update parent status.
//!      - Recurse (treat parent as the changed child).
//!
//! ## Design Decisions
//!
//! - **No Downward Propagation**: Setting a parent to "Done" acts as a milestone check, not a batch operation.
//!   We preserves the individual states of children.
//! - **Eventual Consistency**: While manual overrides allow temporary inconsistency, the system trends towards
//!   consistency as children are updated.
//! - **Storage**: Status is a persistent field in `Metadata`, not just a runtime calculation. This allows
//!   for the manual overrides to persist until challenged.
//!

use crate::error::Result;
use crate::model::{Scope, TodoStatus};
use crate::store::Bucket;
use crate::store::DataStore;
use uuid::Uuid;

/// Propagates status changes upwards from a specific child pad.
///
/// This function should be called whenever a pad is:
/// - Created (if it has a parent)
/// - Deleted (if it had a parent)
/// - Updated (if its status, delete state, or parent changed)
///
/// It recursively updates parents until the root or until no change occurs.
pub fn propagate_status_change<S: DataStore>(
    store: &mut S,
    scope: Scope,
    child_parent_id: Option<Uuid>,
) -> Result<()> {
    let mut current_parent_id = child_parent_id;

    while let Some(parent_id) = current_parent_id {
        // 1. Get the parent
        let mut parent_pad = match store.get_pad(&parent_id, scope, Bucket::Active) {
            Ok(p) => p,
            Err(_) => {
                // Parent might be deleted or missing, stop propagation
                break;
            }
        };

        // 2. Get all children (siblings of the original child)
        // We need efficient lookups. Store lists all pads?
        // Optimally, store would support `get_children(parent_id)`.
        // For now, we iterate (inefficient but safe for "basic level").
        // TODO: Optimize store query for children.
        let all_pads = store.list_pads(scope, Bucket::Active)?;
        let children: Vec<&crate::model::Pad> = all_pads
            .iter()
            .filter(|p| p.metadata.parent_id == Some(parent_id))
            .collect();

        if children.is_empty() {
            // No active children? Status is not derived. Stop.
            // Or should it revert to Planned? Spec doesn't say.
            break;
        }

        // 3. Calculate derived status
        let derived = calculate_status(&children);

        // 4. Update if needed
        if parent_pad.metadata.status != derived {
            // println!("Updating parent {} status from {:?} to {:?}", parent_pad.metadata.title, parent_pad.metadata.status, derived);
            parent_pad.metadata.status = derived;
            parent_pad.metadata.updated_at = chrono::Utc::now();
            store.save_pad(&parent_pad, scope, Bucket::Active)?;

            // Recurse up
            current_parent_id = parent_pad.metadata.parent_id;
        } else {
            // No change, stop propagation
            break;
        }
    }

    Ok(())
}

/// Calculates the status of a parent based on its children.
fn calculate_status(children: &[&crate::model::Pad]) -> TodoStatus {
    let all_done = children
        .iter()
        .all(|p| p.metadata.status == TodoStatus::Done);
    let all_planned = children
        .iter()
        .all(|p| p.metadata.status == TodoStatus::Planned);

    if all_done {
        TodoStatus::Done
    } else if all_planned {
        TodoStatus::Planned
    } else {
        // Mixed state (some Done, some Planned, or any InProgress)
        TodoStatus::InProgress
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Pad, TodoStatus};
    use crate::store::bucketed::BucketedStore;
    use crate::store::mem_backend::MemBackend;

    fn make_pad(title: &str, status: TodoStatus) -> Pad {
        let mut p = Pad::new(title.to_string(), "".to_string());
        p.metadata.status = status;
        p
    }

    #[test]
    fn test_propagate_all_planned() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let parent = make_pad("Parent", TodoStatus::Done); // Wrong status initially
        let mut child1 = make_pad("Child1", TodoStatus::Planned);
        let mut child2 = make_pad("Child2", TodoStatus::Planned);

        // Setup hierarchy
        let parent_id = parent.metadata.id;
        child1.metadata.parent_id = Some(parent_id);
        child2.metadata.parent_id = Some(parent_id);

        store
            .save_pad(&parent, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&child1, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&child2, Scope::Project, Bucket::Active)
            .unwrap();

        // Propagate from child1
        propagate_status_change(&mut store, Scope::Project, Some(parent_id)).unwrap();

        // Parent should become Planned
        let updated_parent = store
            .get_pad(&parent_id, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(updated_parent.metadata.status, TodoStatus::Planned);
    }

    #[test]
    fn test_propagate_all_done() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let parent = make_pad("Parent", TodoStatus::Planned);
        let mut child1 = make_pad("Child1", TodoStatus::Done);
        let mut child2 = make_pad("Child2", TodoStatus::Done);

        let parent_id = parent.metadata.id;
        child1.metadata.parent_id = Some(parent_id);
        child2.metadata.parent_id = Some(parent_id);

        store
            .save_pad(&parent, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&child1, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&child2, Scope::Project, Bucket::Active)
            .unwrap();

        propagate_status_change(&mut store, Scope::Project, Some(parent_id)).unwrap();

        let updated_parent = store
            .get_pad(&parent_id, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(updated_parent.metadata.status, TodoStatus::Done);
    }

    #[test]
    fn test_propagate_mixed_done_planned() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let parent = make_pad("Parent", TodoStatus::Planned);
        let mut child1 = make_pad("Child1", TodoStatus::Done);
        let mut child2 = make_pad("Child2", TodoStatus::Planned);

        let parent_id = parent.metadata.id;
        child1.metadata.parent_id = Some(parent_id);
        child2.metadata.parent_id = Some(parent_id);

        store
            .save_pad(&parent, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&child1, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&child2, Scope::Project, Bucket::Active)
            .unwrap();

        propagate_status_change(&mut store, Scope::Project, Some(parent_id)).unwrap();

        let updated_parent = store
            .get_pad(&parent_id, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(updated_parent.metadata.status, TodoStatus::InProgress);
    }

    #[test]
    fn test_propagate_ignores_deleted_children() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let parent = make_pad("Parent", TodoStatus::Planned);
        let mut child1 = make_pad("Child1", TodoStatus::Done);
        let mut child2 = make_pad("Child2", TodoStatus::Planned);

        let parent_id = parent.metadata.id;
        child1.metadata.parent_id = Some(parent_id);
        child2.metadata.parent_id = Some(parent_id);

        store
            .save_pad(&parent, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&child1, Scope::Project, Bucket::Active)
            .unwrap();
        // Child2 is in the Deleted bucket — should be ignored by propagation
        store
            .save_pad(&child2, Scope::Project, Bucket::Deleted)
            .unwrap();

        propagate_status_change(&mut store, Scope::Project, Some(parent_id)).unwrap();

        // Only child1 (Done) counts -> Parent should be Done!
        let updated_parent = store
            .get_pad(&parent_id, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(updated_parent.metadata.status, TodoStatus::Done);
    }

    #[test]
    fn test_propagate_recursive() {
        let mut store = BucketedStore::new(
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
            MemBackend::new(),
        );
        let mut grandparent = make_pad("GP", TodoStatus::Planned);
        let mut parent = make_pad("Parent", TodoStatus::Planned);
        let mut child = make_pad("Child", TodoStatus::Done);

        let gp_id = grandparent.metadata.id;
        let p_id = parent.metadata.id;

        grandparent.metadata.parent_id = None;
        parent.metadata.parent_id = Some(gp_id);
        child.metadata.parent_id = Some(p_id);

        store
            .save_pad(&grandparent, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&parent, Scope::Project, Bucket::Active)
            .unwrap();
        store
            .save_pad(&child, Scope::Project, Bucket::Active)
            .unwrap();

        // Trigger on child's parent
        propagate_status_change(&mut store, Scope::Project, Some(p_id)).unwrap();

        // Check parent (should be Done)
        let updated_p = store
            .get_pad(&p_id, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(updated_p.metadata.status, TodoStatus::Done);

        // Check grandparent (should be Done)
        let updated_gp = store
            .get_pad(&gp_id, Scope::Project, Bucket::Active)
            .unwrap();
        assert_eq!(updated_gp.metadata.status, TodoStatus::Done);
    }
}
