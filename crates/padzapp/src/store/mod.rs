//! # Storage Layer
//!
//! This module defines the storage abstraction for padz. The [`DataStore`] trait
//! allows the application to work with different storage backends.
//!
//! ## Hybrid Store Architecture
//!
//! Padz maintains a split-brain model:
//! 1. **Truth**: Text files on disk.
//! 2. **Cache**: A JSON metadata "database" (`data.json`).
//!
//! The system assumes the cache is *always potentially dirty* and self-heals lazily.
//!
//! ### Philosophy
//! - **Files are Truth**: If a file exists in `.padz/`, it is a valid pad. If deleted, the pad is gone.
//! - **Lazy Reconciliation**: The database is updated whenever a "read" operation occurs.
//! - **Robustness**: Operations are robust against process termination. The editor saves directly to disk.
//!
//! ## Reconciliation Logic
//!
//! The `sync` process runs automatically before listing pads:
//!
//! 1. **Orphan Adoption**: `pad-X.txt` exists but `X` not in DB → Parse and add to DB.
//! 2. **Zombie Cleanup**: `X` in DB but `pad-X.txt` missing → Remove from DB.
//! 3. **Staleness Check**: File `mtime` > DB `updated_at` → Re-parse to update cached title.
//! 4. **Garbage Collection**: Empty/whitespace-only file → Delete file and DB entry.
//!
//! ## Deletion Lifecycle
//!
//! - **Soft Delete**: Sets `is_deleted = true` in Metadata. File remains on disk for undo.
//! - **Purge**: Permanently removes both file and metadata entry.
//!
//! ## File Extension Handling
//!
//! - New pads use the configured `file-ext` (default `.txt`)
//! - When reading, tries configured extension first, falls back to `.txt`
//! - Mixed extensions are supported gracefully
//!
//! ## Metadata Fields
//!
//! The `data.json` stores `HashMap<Uuid, Metadata>` with:
//! - `id`, `created_at`, `updated_at`: Identity and timestamps
//! - `is_pinned`, `pinned_at`: Pin state
//! - `is_deleted`, `deleted_at`: Soft-delete state
//! - `delete_protected`: Protection flag
//! - `title`: Cached title for fast listing
//!
//! ## Implementations
//!
//! - [`fs::FileStore`]: Production implementation of the Lazy Reconciler architecture.
//! - [`memory::InMemoryStore`]: For testing logic without filesystem I/O.
//!
//! ## Storage Layout
//!
//! ```text
//! .padz/
//! ├── data.json           # Metadata Cache
//! ├── config.json         # Scope configuration
//! └── pad-{uuid}.{ext}    # Pad content files
//! ```

use crate::error::Result;
use crate::model::{Pad, Scope};
use std::path::PathBuf;
use uuid::Uuid;

pub mod backend;
pub mod fs;
pub mod fs_backend;
pub mod mem_backend;
pub mod memory;
pub mod pad_store;

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
