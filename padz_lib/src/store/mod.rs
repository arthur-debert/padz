use crate::error::Result;
use crate::model::{Pad, Scope};
use uuid::Uuid;

pub mod memory;
pub mod fs;
// pub mod fs; // Will be added later

/// Abstract interface for Pad storage.
/// Designed to be agnostic of the underlying storage mechanism (File, DB, Memory).
pub trait DataStore {
    /// Save a pad (create or update)
    fn save_pad(&mut self, pad: &Pad, scope: Scope) -> Result<()>;

    /// Get a pad by ID
    fn get_pad(&self, id: &Uuid, scope: Scope) -> Result<Pad>;

    /// List all pads in a given scope
    fn list_pads(&self, scope: Scope) -> Result<Vec<Pad>>;

    /// Delete a pad (soft delete is handled by logic layer updates, this is physical removal
    /// or just updating the record depending on impl, but usually store just saves what it gets.
    /// However, if we want to delete a file, we might need a dedicated method).
    /// Let's assume for now `save_pad` handles updates including soft-delete flags.
    /// This method is for PERMANENT deletion (nuke/flush).
    fn delete_pad(&mut self, id: &Uuid, scope: Scope) -> Result<()>;
}
