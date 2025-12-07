//! # Storage Layer
//!
//! This module defines the storage abstraction for padz. The [`DataStore`] trait
//! allows the application to work with different storage backends.
//!
//! ## Design Rationale
//!
//! Storage is abstracted behind a trait to:
//! - Enable **testing** with `InMemoryStore` (no filesystem needed)
//! - Allow **future backends** (database, cloud, etc.) without changing core logic
//! - Keep business logic **decoupled** from persistence details
//!
//! ## Implementations
//!
//! - [`fs::FileStore`]: Production file-based storage
//!   - Metadata stored in `data.json`
//!   - Pad content in individual files: `pad-{uuid}.{ext}`
//!   - Supports configurable file extensions
//!
//! - [`memory::InMemoryStore`]: In-memory storage for testing
//!   - No persistence
//!   - Fast, isolated test execution
//!
//! ## Scope Pattern
//!
//! All operations take a [`Scope`] parameter:
//! - `Scope::Project`: Local `.padz/` directory in current project
//! - `Scope::Global`: User-wide storage (`~/.local/share/padz/padz/`)
//!
//! This allows pads to be scoped per-project or shared globally.
//!
//! ## Storage Format
//!
//! For `FileStore`:
//! ```text
//! .padz/
//! ├── data.json           # Metadata for all pads (JSON array)
//! ├── pad-{uuid}.txt      # Individual pad content files
//! └── config.json         # Scope configuration
//! ```
//!
//! Metadata and content are stored separately so listing pads doesn't require
//! reading all content files.

use crate::error::Result;
use crate::model::{Pad, Scope};
use std::path::PathBuf;
use uuid::Uuid;

pub mod fs;
pub mod memory;

/// Report from the `doctor` operation.
#[derive(Debug, Default)]
pub struct DoctorReport {
    pub fixed_missing_files: usize,
    pub recovered_files: usize,
    pub fixed_content_files: usize,
}

/// Abstract interface for pad storage.
///
/// Implementations must handle persistence, retrieval, and consistency
/// for pads within a given scope.
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

    /// Verify and fix consistency issues
    fn doctor(&mut self, scope: Scope) -> Result<DoctorReport>;
}
