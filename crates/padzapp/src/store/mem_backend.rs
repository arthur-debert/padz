use super::backend::StorageBackend;
use crate::error::{PadzError, Result};
use crate::model::{Metadata, Scope};
use crate::tags::TagEntry;
use chrono::{DateTime, Utc};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone)]
struct ContentEntry {
    text: String,
    mtime: DateTime<Utc>,
}

/// In-memory storage backend for testing.
///
/// Uses `RefCell` for interior mutability since padz is single-threaded.
/// This avoids the overhead of `RwLock` while still allowing the
/// `StorageBackend` trait to use `&self` for all methods.
pub struct MemBackend {
    index: RefCell<HashMap<Scope, HashMap<Uuid, Metadata>>>,
    tags: RefCell<HashMap<Scope, Vec<TagEntry>>>,
    content: RefCell<HashMap<(Scope, Uuid), ContentEntry>>,
    simulate_write_error: RefCell<bool>,
}

impl Default for MemBackend {
    fn default() -> Self {
        Self {
            index: RefCell::new(HashMap::new()),
            tags: RefCell::new(HashMap::new()),
            content: RefCell::new(HashMap::new()),
            simulate_write_error: RefCell::new(false),
        }
    }
}

impl MemBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable write error simulation for testing error handling.
    pub fn set_simulate_write_error(&self, simulate: bool) {
        *self.simulate_write_error.borrow_mut() = simulate;
    }

    /// Test helper to set mtime directly for staleness testing.
    /// Returns true if the entry existed and was updated.
    pub fn set_content_mtime(&self, id: &Uuid, scope: Scope, mtime: DateTime<Utc>) -> bool {
        let mut content = self.content.borrow_mut();
        if let Some(entry) = content.get_mut(&(scope, *id)) {
            entry.mtime = mtime;
            true
        } else {
            false
        }
    }
}

impl StorageBackend for MemBackend {
    fn load_index(&self, scope: Scope) -> Result<HashMap<Uuid, Metadata>> {
        let index = self.index.borrow();
        Ok(index.get(&scope).cloned().unwrap_or_default())
    }

    fn save_index(&self, scope: Scope, new_index: &HashMap<Uuid, Metadata>) -> Result<()> {
        if *self.simulate_write_error.borrow() {
            return Err(PadzError::Store("Simulated write error".to_string()));
        }
        let mut index = self.index.borrow_mut();
        index.insert(scope, new_index.clone());
        Ok(())
    }

    fn load_tags(&self, scope: Scope) -> Result<Vec<TagEntry>> {
        let tags = self.tags.borrow();
        Ok(tags.get(&scope).cloned().unwrap_or_default())
    }

    fn save_tags(&self, scope: Scope, new_tags: &[TagEntry]) -> Result<()> {
        if *self.simulate_write_error.borrow() {
            return Err(PadzError::Store("Simulated write error".to_string()));
        }
        let mut tags = self.tags.borrow_mut();
        tags.insert(scope, new_tags.to_vec());
        Ok(())
    }

    fn read_content(&self, id: &Uuid, scope: Scope) -> Result<Option<String>> {
        let content = self.content.borrow();
        Ok(content.get(&(scope, *id)).map(|e| e.text.clone()))
    }

    fn write_content(&self, id: &Uuid, scope: Scope, text: &str) -> Result<()> {
        if *self.simulate_write_error.borrow() {
            return Err(PadzError::Store("Simulated write error".to_string()));
        }

        let mut content = self.content.borrow_mut();
        content.insert(
            (scope, *id),
            ContentEntry {
                text: text.to_string(),
                mtime: Utc::now(),
            },
        );
        Ok(())
    }

    fn delete_content(&self, id: &Uuid, scope: Scope) -> Result<()> {
        let mut content = self.content.borrow_mut();
        content.remove(&(scope, *id));
        Ok(())
    }

    fn list_content_ids(&self, scope: Scope) -> Result<Vec<Uuid>> {
        let content = self.content.borrow();
        Ok(content
            .keys()
            .filter(|(s, _)| *s == scope)
            .map(|(_, id)| *id)
            .collect())
    }

    fn content_mtime(&self, id: &Uuid, scope: Scope) -> Result<Option<DateTime<Utc>>> {
        let content = self.content.borrow();
        Ok(content.get(&(scope, *id)).map(|e| e.mtime))
    }

    fn content_path(&self, id: &Uuid, _scope: Scope) -> Result<PathBuf> {
        Ok(PathBuf::from(format!("memory://pad-{}", id)))
    }

    fn scope_available(&self, _scope: Scope) -> bool {
        true
    }
}
