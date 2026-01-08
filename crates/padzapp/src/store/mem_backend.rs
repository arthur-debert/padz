use super::backend::StorageBackend;
use crate::error::{PadzError, Result};
use crate::model::{Metadata, Scope};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;
use uuid::Uuid;

#[derive(Clone)]
struct ContentEntry {
    text: String,
    mtime: DateTime<Utc>,
}

#[derive(Default)]
pub struct MemBackend {
    index: RwLock<HashMap<Scope, HashMap<Uuid, Metadata>>>,
    content: RwLock<HashMap<(Scope, Uuid), ContentEntry>>,
    pub simulate_write_error: bool,
}

impl MemBackend {
    pub fn new() -> Self {
        Self::default()
    }

    // Test helper to set mtime directly
    pub fn set_content_mtime(&self, id: &Uuid, scope: Scope, mtime: DateTime<Utc>) {
        if let Ok(mut content) = self.content.write() {
            if let Some(entry) = content.get_mut(&(scope, *id)) {
                entry.mtime = mtime;
            }
        }
    }
}

impl StorageBackend for MemBackend {
    fn load_index(&self, scope: Scope) -> Result<HashMap<Uuid, Metadata>> {
        let index = self
            .index
            .read()
            .map_err(|_| PadzError::Store("Lock poisoned".to_string()))?;
        Ok(index.get(&scope).cloned().unwrap_or_default())
    }

    fn save_index(&self, scope: Scope, new_index: &HashMap<Uuid, Metadata>) -> Result<()> {
        if self.simulate_write_error {
            return Err(PadzError::Store("Simulated write error".to_string()));
        }
        let mut index = self
            .index
            .write()
            .map_err(|_| PadzError::Store("Lock poisoned".to_string()))?;
        index.insert(scope, new_index.clone());
        Ok(())
    }

    fn read_content(&self, id: &Uuid, scope: Scope) -> Result<Option<String>> {
        let content = self
            .content
            .read()
            .map_err(|_| PadzError::Store("Lock poisoned".to_string()))?;
        Ok(content.get(&(scope, *id)).map(|e| e.text.clone()))
    }

    fn write_content(&self, id: &Uuid, scope: Scope, text: &str) -> Result<()> {
        if self.simulate_write_error {
            return Err(PadzError::Store("Simulated write error".to_string()));
        }

        let mut content = self
            .content
            .write()
            .map_err(|_| PadzError::Store("Lock poisoned".to_string()))?;
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
        let mut content = self
            .content
            .write()
            .map_err(|_| PadzError::Store("Lock poisoned".to_string()))?;
        content.remove(&(scope, *id));
        Ok(())
    }

    fn list_content_ids(&self, scope: Scope) -> Result<Vec<Uuid>> {
        let content = self
            .content
            .read()
            .map_err(|_| PadzError::Store("Lock poisoned".to_string()))?;
        Ok(content
            .keys()
            .filter(|(s, _)| *s == scope)
            .map(|(_, id)| *id)
            .collect())
    }

    fn content_mtime(&self, id: &Uuid, scope: Scope) -> Result<Option<DateTime<Utc>>> {
        let content = self
            .content
            .read()
            .map_err(|_| PadzError::Store("Lock poisoned".to_string()))?;
        Ok(content.get(&(scope, *id)).map(|e| e.mtime))
    }

    fn content_path(&self, id: &Uuid, _scope: Scope) -> PathBuf {
        PathBuf::from(format!("memory://pad-{}", id))
    }

    fn scope_available(&self, _scope: Scope) -> bool {
        true
    }
}
