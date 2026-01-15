use crate::error::Result;
use crate::model::{Metadata, Scope};
use crate::tags::TagEntry;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use uuid::Uuid;

/// Abstract interface for raw storage I/O.
/// This trait handles the "how" of storage (filesystem vs memory),
/// while PadStore handles the "what" (business logic, sync, doctor).
pub trait StorageBackend {
    // --- Index Operations ---

    /// Load the metadata index (data.json)
    fn load_index(&self, scope: Scope) -> Result<HashMap<Uuid, Metadata>>;

    /// Save the metadata index
    fn save_index(&self, scope: Scope, index: &HashMap<Uuid, Metadata>) -> Result<()>;

    // --- Tag Registry Operations ---

    /// Load the tag registry (tags.json)
    fn load_tags(&self, scope: Scope) -> Result<Vec<TagEntry>>;

    /// Save the tag registry
    fn save_tags(&self, scope: Scope, tags: &[TagEntry]) -> Result<()>;

    // --- Content Operations ---

    /// Read raw content string for a pad.
    /// Returns Ok(None) if the file does not exist (useful for zombie detection).
    /// Returns Err only on actual I/O errors (permissions, disk failure).
    fn read_content(&self, id: &Uuid, scope: Scope) -> Result<Option<String>>;

    /// Write content to storage.
    /// MUST be atomic (e.g. write to tmp then rename) to avoid partial writes.
    fn write_content(&self, id: &Uuid, scope: Scope, content: &str) -> Result<()>;

    /// Delete content file.
    fn delete_content(&self, id: &Uuid, scope: Scope) -> Result<()>;

    // --- Discovery & Metadata ---

    /// List all content IDs found in storage (for sync/reconciliation).
    fn list_content_ids(&self, scope: Scope) -> Result<Vec<Uuid>>;

    /// Get modification time of the content file.
    fn content_mtime(&self, id: &Uuid, scope: Scope) -> Result<Option<DateTime<Utc>>>;

    // --- Paths & Capabilities ---

    /// Get the "file path" for the content.
    /// For FsBackend, this is the real path. For MemBackend, a virtual path.
    /// Returns Err if the scope is not available.
    fn content_path(&self, id: &Uuid, scope: Scope) -> Result<PathBuf>;

    /// Check if a scope is available (e.g. project root exists).
    fn scope_available(&self, scope: Scope) -> bool;
}
