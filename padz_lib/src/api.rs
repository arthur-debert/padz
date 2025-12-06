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

        let pads = api.list_pads(Scope::Project).unwrap();
        // Reverse chronological: B=1 (newest), A=2 (oldest)

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
}
