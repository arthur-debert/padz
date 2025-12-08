//! # Storage Layer
//!
//! This module defines the storage abstraction for padz. The [`DataStore`] trait
//! allows the application to work with different storage backends.
//!
//! ## Architecture: Lazy Reconciler
//!
//! Padz uses a **file-system-centric** architecture where the file system is the ultimate
//! source of truth. The database (`data.json`) acts as a lightweight metadata cache/registry.
//!
//! ### Philosophy
//! - **Files are Truth**: If a file exists in `.padz/`, it is a valid pad. If it is deleted, the pad is gone.
//! - **Lazy Reconciliation**: The database is updated (reconciled) lazily whenever a "read" operation (like `list`) occurs.
//! - **Robustness**: Operations are robust against process termination. The editor saves content directly to disk.
//!   If `padz` crashes or is killed, the file remains safe on disk and will be picked up by the next reconciliation.
//!
//! ### Reconciliation Logic
//!
//! The reconciliation process (`sync`) runs automatically before listing pads. It handles three key scenarios:
//!
//! 1. **Zombie Files** (Database Clean-up)
//!    - **Condition**: File is listed in `data.json` but does not exist on disk.
//!    - **Action**: The database entry is removed.
//!
//! 2. **Orphaned Files** (Discovery)
//!    - **Condition**: File exists on disk (`pad-{uuid}.txt`) but has no entry in `data.json`.
//!    - **Action**: The file is parsed, and a new entry is added to the database.
//!
//! 3. **Empty Files** (Garbage Collection)
//!    - **Condition**: A file on disk has empty or whitespace-only content.
//!    - **Action**: The file is deleted from disk, and its database entry is removed.
//!
//! ### Safety & Recovery
//!
//! This design provides strong safety guarantees:
//! - **Crash Recovery**: Since the editor operates directly on the file, saving works independently of the `padz` process.
//!   Any content saved by the editor is "safe" and will be discovered by the Reconciler.
//! - **External Edits**: Users can manually create or edit files in `.padz/`. The system accepts these as valid changes.
//!
//! ## Implementations
//!
//! - [`fs::FileStore`]: Production implementation of the Lazy Reconciler architecture.
//! - [`memory::InMemoryStore`]: For testing logic without filesystem I/O.
//!
//! ## Storage Format
//!
//! ```text
//! .padz/
//! ├── data.json           # Metadata Cache (Registry, pinned status, cached titles)
//! ├── pad-{uuid}.{ext}    # Source of Truth: Pad content
//! └── config.json         # Scope configuration
//! ```

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
