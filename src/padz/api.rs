use crate::error::{PadzError, Result};
use crate::index::{DisplayIndex, DisplayPad, index_pads};
use crate::model::{Pad, Scope};
use crate::store::DataStore;
use std::path::PathBuf;
use uuid::Uuid;

pub struct PadzApi<S: DataStore> {
    store: S,
}

impl<S: DataStore> PadzApi<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub fn create_pad(&mut self, title: String, content: String, scope: Scope) -> Result<Pad> {
        let pad = Pad::new(title, content);
        self.store.save_pad(&pad, scope)?;
        Ok(pad)
    }

    pub fn list_pads(&self, scope: Scope) -> Result<Vec<DisplayPad>> {
        let pads = self.store.list_pads(scope)?;
        Ok(index_pads(pads))
    }

    /// Resolves a DisplayIndex (e.g. "1", "p1") to a UUID.
    /// This requires loading ALL pads to re-calculate the indexes.
    /// This is the "stateless" part of PADZ.
    pub fn resolve_index(&self, index: &DisplayIndex, scope: Scope) -> Result<Uuid> {
        let pads = self.store.list_pads(scope)?;
        let indexed = index_pads(pads);

        for display_pad in indexed {
            if &display_pad.index == index {
                return Ok(display_pad.pad.metadata.id);
            }
        }

        // Error message construction helper
        let msg = format!("Index {} not found in current scope", index);
        Err(PadzError::Api(msg))
    }

    /// Resolves multiple DisplayIndexes to UUIDs in a single pass.
    /// This is critical for multi-pad commands: all indexes must be resolved
    /// BEFORE any mutations occur, otherwise indexes would shift during execution.
    ///
    /// Returns an error if ANY index fails to resolve.
    pub fn resolve_indexes(&self, indexes: &[DisplayIndex], scope: Scope) -> Result<Vec<Uuid>> {
        let pads = self.store.list_pads(scope)?;
        let indexed = index_pads(pads);

        let mut uuids = Vec::with_capacity(indexes.len());

        for index in indexes {
            let uuid = indexed
                .iter()
                .find(|dp| &dp.index == index)
                .map(|dp| dp.pad.metadata.id)
                .ok_or_else(|| {
                    PadzError::Api(format!("Index {} not found in current scope", index))
                })?;
            uuids.push(uuid);
        }

        Ok(uuids)
    }

    /// Batch delete pads by UUIDs (already resolved).
    pub fn delete_pads_by_uuids(&mut self, uuids: &[Uuid], scope: Scope) -> Result<Vec<Pad>> {
        let mut results = Vec::with_capacity(uuids.len());
        for uuid in uuids {
            let mut pad = self.store.get_pad(uuid, scope)?;
            pad.metadata.is_deleted = true;
            pad.metadata.deleted_at = Some(chrono::Utc::now());
            self.store.save_pad(&pad, scope)?;
            results.push(pad);
        }
        Ok(results)
    }

    /// Batch pin pads by UUIDs (already resolved).
    pub fn pin_pads_by_uuids(&mut self, uuids: &[Uuid], scope: Scope) -> Result<Vec<Pad>> {
        let mut results = Vec::with_capacity(uuids.len());
        for uuid in uuids {
            let mut pad = self.store.get_pad(uuid, scope)?;
            pad.metadata.is_pinned = true;
            pad.metadata.pinned_at = Some(chrono::Utc::now());
            self.store.save_pad(&pad, scope)?;
            results.push(pad);
        }
        Ok(results)
    }

    /// Batch unpin pads by UUIDs (already resolved).
    pub fn unpin_pads_by_uuids(&mut self, uuids: &[Uuid], scope: Scope) -> Result<Vec<Pad>> {
        let mut results = Vec::with_capacity(uuids.len());
        for uuid in uuids {
            let mut pad = self.store.get_pad(uuid, scope)?;
            pad.metadata.is_pinned = false;
            pad.metadata.pinned_at = None;
            self.store.save_pad(&pad, scope)?;
            results.push(pad);
        }
        Ok(results)
    }

    /// Get multiple pads by UUIDs (already resolved).
    pub fn get_pads_by_uuids(
        &self,
        uuids: &[Uuid],
        indexes: &[DisplayIndex],
        scope: Scope,
    ) -> Result<Vec<DisplayPad>> {
        let mut results = Vec::with_capacity(uuids.len());
        for (uuid, index) in uuids.iter().zip(indexes.iter()) {
            let pad = self.store.get_pad(uuid, scope)?;
            results.push(DisplayPad {
                pad,
                index: index.clone(),
            });
        }
        Ok(results)
    }

    /// Get multiple pad paths by UUIDs (already resolved).
    pub fn get_pad_paths_by_uuids(&self, uuids: &[Uuid], scope: Scope) -> Result<Vec<PathBuf>> {
        let mut results = Vec::with_capacity(uuids.len());
        for uuid in uuids {
            let path = self.store.get_pad_path(uuid, scope)?;
            results.push(path);
        }
        Ok(results)
    }

    /// Get a pad by UUID (direct store access).
    pub fn get_pad_by_uuid(&self, uuid: &Uuid, scope: Scope) -> Result<Pad> {
        self.store.get_pad(uuid, scope)
    }

    /// Save a pad (direct store access).
    pub fn save_pad(&mut self, pad: &Pad, scope: Scope) -> Result<()> {
        self.store.save_pad(pad, scope)
    }

    /// Update a pad by UUID.
    pub fn update_pad_by_uuid(
        &mut self,
        uuid: &Uuid,
        title: String,
        content: String,
        scope: Scope,
    ) -> Result<Pad> {
        let mut pad = self.store.get_pad(uuid, scope)?;
        pad.metadata.title = title;
        pad.metadata.updated_at = chrono::Utc::now();
        pad.content = content;
        self.store.save_pad(&pad, scope)?;
        Ok(pad)
    }

    pub fn get_pad(&self, index: &DisplayIndex, scope: Scope) -> Result<DisplayPad> {
        let uuid = self.resolve_index(index, scope)?;
        let pad = self.store.get_pad(&uuid, scope)?;

        // We also need the index... but resolve_index just gave us the UUID.
        // We could return (Pad, Index) from resolve, but let's stick to simple composition.
        // Actually, resolve_index loaded everything. If we want to be efficient we might change this,
        // but for now correctness > perf.

        Ok(DisplayPad {
            pad,
            index: index.clone(),
        })
    }

    pub fn delete_pad(&mut self, index: &DisplayIndex, scope: Scope) -> Result<Pad> {
        let uuid = self.resolve_index(index, scope)?;
        let mut pad = self.store.get_pad(&uuid, scope)?;

        pad.metadata.is_deleted = true;
        pad.metadata.deleted_at = Some(chrono::Utc::now());

        self.store.save_pad(&pad, scope)?;
        Ok(pad)
    }

    pub fn pin_pad(&mut self, index: &DisplayIndex, scope: Scope) -> Result<Pad> {
        let uuid = self.resolve_index(index, scope)?;
        let mut pad = self.store.get_pad(&uuid, scope)?;

        pad.metadata.is_pinned = true;
        pad.metadata.pinned_at = Some(chrono::Utc::now());

        self.store.save_pad(&pad, scope)?;
        Ok(pad)
    }

    pub fn unpin_pad(&mut self, index: &DisplayIndex, scope: Scope) -> Result<Pad> {
        let uuid = self.resolve_index(index, scope)?;
        let mut pad = self.store.get_pad(&uuid, scope)?;

        pad.metadata.is_pinned = false;
        pad.metadata.pinned_at = None;

        self.store.save_pad(&pad, scope)?;
        Ok(pad)
    }

    pub fn update_pad(
        &mut self,
        index: &DisplayIndex,
        title: String,
        content: String,
        scope: Scope,
    ) -> Result<Pad> {
        let uuid = self.resolve_index(index, scope)?;
        let mut pad = self.store.get_pad(&uuid, scope)?;

        pad.metadata.title = title;
        pad.metadata.updated_at = chrono::Utc::now();
        pad.content = content;

        self.store.save_pad(&pad, scope)?;
        Ok(pad)
    }

    pub fn get_pad_path(&self, index: &DisplayIndex, scope: Scope) -> Result<PathBuf> {
        let uuid = self.resolve_index(index, scope)?;
        self.store.get_pad_path(&uuid, scope)
    }

    pub fn search_pads(&self, term: &str, scope: Scope) -> Result<Vec<DisplayPad>> {
        let pads = self.store.list_pads(scope)?;
        let indexed = index_pads(pads);

        let term_lower = term.to_lowercase();

        let mut matches: Vec<(DisplayPad, u8)> = indexed
            .into_iter()
            .filter_map(|dp| {
                let title_lower = dp.pad.metadata.title.to_lowercase();
                let content_lower = dp.pad.content.to_lowercase();

                let score = if title_lower == term_lower {
                    1
                } else if title_lower.contains(&term_lower) {
                    2
                } else if content_lower.contains(&term_lower) {
                    3
                } else {
                    return None;
                };

                Some((dp, score))
            })
            .collect();

        matches.sort_by(|(a, score_a), (b, score_b)| match score_a.cmp(score_b) {
            std::cmp::Ordering::Equal => {
                let len_a = a.pad.metadata.title.len();
                let len_b = b.pad.metadata.title.len();
                match len_a.cmp(&len_b) {
                    std::cmp::Ordering::Equal => {
                        a.pad.metadata.created_at.cmp(&b.pad.metadata.created_at)
                    }
                    ord => ord,
                }
            }
            ord => ord,
        });

        Ok(matches.into_iter().map(|(dp, _)| dp).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::memory::InMemoryStore;
    // Ensure fixtures are public
    // If fixtures are not pub, I might need to adjust memory.rs visibility or just use InMemoryStore directly.
    // memory.rs has `pub mod fixtures` but it's inside `#[cfg(any(test, feature = "test_utils"))]`.
    // I need to ensure `test_utils` is enabled or I am running tests. Since this is `#[cfg(test)]`, it should be fine.

    #[test]
    fn test_create_and_list() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        let pad = api
            .create_pad("My Pad".to_string(), "Content".to_string(), Scope::Project)
            .unwrap();
        assert_eq!(pad.metadata.title, "My Pad");

        let pads = api.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 1);
        assert_eq!(pads[0].pad.metadata.id, pad.metadata.id);
        assert_eq!(pads[0].index, DisplayIndex::Regular(1));
    }

    #[test]
    fn test_pinning_reorders_indexes() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        api.create_pad("A".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("B".to_string(), "".to_string(), Scope::Project)
            .unwrap();

        let idx_1 = DisplayIndex::Regular(1); // B is now index 1
        api.pin_pad(&idx_1, Scope::Project).unwrap();

        let pads_after = api.list_pads(Scope::Project).unwrap();
        // Now B should be p1 (pinned), and also in regular list.
        // A should remain at index 2.

        let p_b = pads_after
            .iter()
            .find(|p| p.pad.metadata.title == "B")
            .unwrap();
        assert_eq!(p_b.index, DisplayIndex::Pinned(1));

        let p_a = pads_after
            .iter()
            .find(|p| p.pad.metadata.title == "A")
            .unwrap();
        assert_eq!(p_a.index, DisplayIndex::Regular(2));
    }

    #[test]
    fn test_search_ranking() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        api.create_pad(
            "Foo Bar".to_string(),
            "Just content".to_string(),
            Scope::Project,
        )
        .unwrap(); // Partial
        api.create_pad(
            "Bar".to_string(),
            "Matches match".to_string(),
            Scope::Project,
        )
        .unwrap(); // Exact
        api.create_pad(
            "Zebra".to_string(),
            "Contains Bar in content".to_string(),
            Scope::Project,
        )
        .unwrap(); // Content match

        let results = api.search_pads("Bar", Scope::Project).unwrap();

        // Expect:
        // 1. "Bar" (Exact)
        // 2. "Foo Bar" (Partial)
        // 3. "Zebra" (Content)

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].pad.metadata.title, "Bar");
        assert_eq!(results[1].pad.metadata.title, "Foo Bar");
        assert_eq!(results[2].pad.metadata.title, "Zebra");
    }

    #[test]
    fn test_delete_and_restore() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);
        api.create_pad("To Delete".to_string(), "".to_string(), Scope::Project)
            .unwrap();

        // Delete
        api.delete_pad(&DisplayIndex::Regular(1), Scope::Project)
            .unwrap();

        let pads = api.list_pads(Scope::Project).unwrap();
        assert_eq!(pads.len(), 1);
        assert!(matches!(pads[0].index, DisplayIndex::Deleted(_)));
    }

    #[test]
    fn test_update_pad() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        api.create_pad(
            "Original Title".to_string(),
            "Original content".to_string(),
            Scope::Project,
        )
        .unwrap();

        let updated = api
            .update_pad(
                &DisplayIndex::Regular(1),
                "New Title".to_string(),
                "New content".to_string(),
                Scope::Project,
            )
            .unwrap();

        assert_eq!(updated.metadata.title, "New Title");
        assert_eq!(updated.content, "New content");

        // Verify it persisted
        let fetched = api
            .get_pad(&DisplayIndex::Regular(1), Scope::Project)
            .unwrap();
        assert_eq!(fetched.pad.metadata.title, "New Title");
        assert_eq!(fetched.pad.content, "New content");
    }

    #[test]
    fn test_resolve_indexes_success() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        // Create 4 pads: newest first ordering means D=1, C=2, B=3, A=4
        api.create_pad("A".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("B".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("C".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("D".to_string(), "".to_string(), Scope::Project)
            .unwrap();

        // Resolve indexes 1 and 3 (D and B)
        let indexes = vec![DisplayIndex::Regular(1), DisplayIndex::Regular(3)];
        let uuids = api.resolve_indexes(&indexes, Scope::Project).unwrap();

        assert_eq!(uuids.len(), 2);

        // Verify the UUIDs correspond to the correct pads
        let pad_d = api
            .get_pad(&DisplayIndex::Regular(1), Scope::Project)
            .unwrap();
        let pad_b = api
            .get_pad(&DisplayIndex::Regular(3), Scope::Project)
            .unwrap();

        assert_eq!(uuids[0], pad_d.pad.metadata.id);
        assert_eq!(uuids[1], pad_b.pad.metadata.id);
    }

    #[test]
    fn test_resolve_indexes_fails_on_invalid() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        api.create_pad("A".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("B".to_string(), "".to_string(), Scope::Project)
            .unwrap();

        // Try to resolve index 1 (valid) and index 99 (invalid)
        let indexes = vec![DisplayIndex::Regular(1), DisplayIndex::Regular(99)];
        let result = api.resolve_indexes(&indexes, Scope::Project);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("99"));
    }

    #[test]
    fn test_multi_delete_resolves_before_mutation() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        // Create 4 pads: D=1, C=2, B=3, A=4 (newest first)
        api.create_pad("A".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("B".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("C".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("D".to_string(), "".to_string(), Scope::Project)
            .unwrap();

        // We want to delete pads 1 and 3 (D and B)
        // If we deleted 1 first, then "3" would point to A instead of B!
        // By resolving upfront, we ensure we get the correct UUIDs.
        let indexes = vec![DisplayIndex::Regular(1), DisplayIndex::Regular(3)];
        let uuids = api.resolve_indexes(&indexes, Scope::Project).unwrap();

        // Now delete using UUIDs
        let deleted = api.delete_pads_by_uuids(&uuids, Scope::Project).unwrap();

        assert_eq!(deleted.len(), 2);
        assert_eq!(deleted[0].metadata.title, "D");
        assert_eq!(deleted[1].metadata.title, "B");

        // Verify the remaining pads are C and A (as deleted)
        let remaining = api.list_pads(Scope::Project).unwrap();
        let regular: Vec<_> = remaining
            .iter()
            .filter(|p| matches!(p.index, DisplayIndex::Regular(_)))
            .collect();
        let deleted_pads: Vec<_> = remaining
            .iter()
            .filter(|p| matches!(p.index, DisplayIndex::Deleted(_)))
            .collect();

        assert_eq!(regular.len(), 2); // C and A remain as regular
        assert_eq!(deleted_pads.len(), 2); // D and B are deleted
    }

    #[test]
    fn test_multi_pin_by_uuids() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        api.create_pad("A".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("B".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("C".to_string(), "".to_string(), Scope::Project)
            .unwrap();

        // Pin indexes 1 and 3 (C and A, since newest first)
        let indexes = vec![DisplayIndex::Regular(1), DisplayIndex::Regular(3)];
        let uuids = api.resolve_indexes(&indexes, Scope::Project).unwrap();
        let pinned = api.pin_pads_by_uuids(&uuids, Scope::Project).unwrap();

        assert_eq!(pinned.len(), 2);
        assert_eq!(pinned[0].metadata.title, "C");
        assert!(pinned[0].metadata.is_pinned);
        assert_eq!(pinned[1].metadata.title, "A");
        assert!(pinned[1].metadata.is_pinned);

        // Verify pinned section has 2 entries
        let all_pads = api.list_pads(Scope::Project).unwrap();
        let pinned_entries: Vec<_> = all_pads
            .iter()
            .filter(|p| matches!(p.index, DisplayIndex::Pinned(_)))
            .collect();
        assert_eq!(pinned_entries.len(), 2);
    }

    #[test]
    fn test_multi_unpin_by_uuids() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        api.create_pad("A".to_string(), "".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("B".to_string(), "".to_string(), Scope::Project)
            .unwrap();

        // Pin both
        let indexes = vec![DisplayIndex::Regular(1), DisplayIndex::Regular(2)];
        let uuids = api.resolve_indexes(&indexes, Scope::Project).unwrap();
        api.pin_pads_by_uuids(&uuids, Scope::Project).unwrap();

        // Now unpin both using pinned indexes
        let pinned_indexes = vec![DisplayIndex::Pinned(1), DisplayIndex::Pinned(2)];
        let pinned_uuids = api
            .resolve_indexes(&pinned_indexes, Scope::Project)
            .unwrap();
        let unpinned = api
            .unpin_pads_by_uuids(&pinned_uuids, Scope::Project)
            .unwrap();

        assert_eq!(unpinned.len(), 2);
        assert!(!unpinned[0].metadata.is_pinned);
        assert!(!unpinned[1].metadata.is_pinned);
    }

    #[test]
    fn test_get_pads_by_uuids() {
        let store = InMemoryStore::new();
        let mut api = PadzApi::new(store);

        api.create_pad("A".to_string(), "content A".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("B".to_string(), "content B".to_string(), Scope::Project)
            .unwrap();
        api.create_pad("C".to_string(), "content C".to_string(), Scope::Project)
            .unwrap();

        let indexes = vec![DisplayIndex::Regular(1), DisplayIndex::Regular(3)];
        let uuids = api.resolve_indexes(&indexes, Scope::Project).unwrap();
        let pads = api
            .get_pads_by_uuids(&uuids, &indexes, Scope::Project)
            .unwrap();

        assert_eq!(pads.len(), 2);
        assert_eq!(pads[0].pad.metadata.title, "C"); // newest = 1
        assert_eq!(pads[1].pad.metadata.title, "A"); // oldest = 3
    }
}
