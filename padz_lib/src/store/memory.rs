use super::DataStore;
use crate::error::{PadzError, Result};
use crate::model::{Pad, Scope};
use std::collections::HashMap;
use uuid::Uuid;

/// In-memory storage for testing and development.
/// Does NOT persist data.
#[derive(Default)]
pub struct InMemoryStore {
    pads: HashMap<(Scope, Uuid), Pad>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl DataStore for InMemoryStore {
    fn save_pad(&mut self, pad: &Pad, scope: Scope) -> Result<()> {
        self.pads.insert((scope, pad.metadata.id), pad.clone());
        Ok(())
    }

    fn get_pad(&self, id: &Uuid, scope: Scope) -> Result<Pad> {
        self.pads
            .get(&(scope, *id))
            .cloned()
            .ok_or(PadzError::PadNotFound(*id))
    }

    fn list_pads(&self, scope: Scope) -> Result<Vec<Pad>> {
        Ok(self
            .pads
            .iter()
            .filter(|((s, _), _)| *s == scope)
            .map(|(_, p)| p.clone())
            .collect())
    }

    fn delete_pad(&mut self, id: &Uuid, scope: Scope) -> Result<()> {
        if self.pads.remove(&(scope, *id)).is_none() {
            return Err(PadzError::PadNotFound(*id));
        }
        Ok(())
    }
}

// --- Test Fixtures ---

#[cfg(any(test, feature = "test_utils"))]
pub mod fixtures {
    use super::*;

    pub struct StoreFixture {
        pub store: InMemoryStore,
    }

    impl Default for StoreFixture {
        fn default() -> Self {
            Self::new()
        }
    }

    impl StoreFixture {
        pub fn new() -> Self {
            Self {
                store: InMemoryStore::new(),
            }
        }

        pub fn with_pads(mut self, count: usize, scope: Scope) -> Self {
            for i in 0..count {
                let title = format!("Test Pad {}", i + 1);
                let content = format!("Content for pad {}", i + 1);
                let pad = Pad::new(title, content);
                self.store.save_pad(&pad, scope).unwrap();
            }
            self
        }

        pub fn with_active_pad(mut self, title: &str, scope: Scope) -> Self {
            let pad = Pad::new(title.to_string(), "Some content".to_string());
            self.store.save_pad(&pad, scope).unwrap();
            self
        }

        pub fn with_pinned_pad(mut self, title: &str, scope: Scope) -> Self {
            let mut pad = Pad::new(title.to_string(), "Pinned content".to_string());
            pad.metadata.is_pinned = true;
            pad.metadata.pinned_at = Some(chrono::Utc::now());
            self.store.save_pad(&pad, scope).unwrap();
            self
        }

        pub fn with_deleted_pad(mut self, title: &str, scope: Scope) -> Self {
            let mut pad = Pad::new(title.to_string(), "Deleted content".to_string());
            pad.metadata.is_deleted = true;
            pad.metadata.deleted_at = Some(chrono::Utc::now());
            self.store.save_pad(&pad, scope).unwrap();
            self
        }
    }
}
