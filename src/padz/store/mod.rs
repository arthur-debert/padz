use crate::error::Result;
use crate::model::{Pad, Scope};
use std::path::PathBuf;
use uuid::Uuid;

pub mod fs;
pub mod memory;

/// Abstract interface for Pad storage.
/// Designed to be agnostic of the underlying storage mechanism (File, DB, Memory).
pub trait DataStore {
    /// Save a pad (create or update)
    fn save_pad(&mut self, pad: &Pad, scope: Scope) -> Result<()>;

    /// Get a pad by ID
    fn get_pad(&self, id: &Uuid, scope: Scope) -> Result<Pad>;

    /// List all pads in a given scope
    fn list_pads(&self, scope: Scope) -> Result<Vec<Pad>>;

    /// Delete a pad permanently
    fn delete_pad(&mut self, id: &Uuid, scope: Scope) -> Result<()>;

    /// Get the file path for a pad (for file-based stores)
    fn get_pad_path(&self, id: &Uuid, scope: Scope) -> Result<PathBuf>;
}
