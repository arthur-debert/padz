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
//! - **Soft Delete**: Moves the pad from the Active bucket to the Deleted bucket.
//! - **Purge**: Permanently removes both file and metadata entry from the Deleted bucket.
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
//! - `delete_protected`: Protection flag
//! - `title`: Cached title for fast listing
//!
//! ## Architecture
//!
//! The store layer is split into two tiers:
//!
//! 1. **[`backend::StorageBackend`]**: Low-level I/O trait (pure read/write operations)
//!    - [`fs_backend::FsBackend`]: Filesystem backend with atomic writes
//!    - [`mem_backend::MemBackend`]: In-memory backend for testing
//!
//! 2. **[`pad_store::PadStore<B>`]**: Business logic layer (sync, doctor, CRUD)
//!    - Implements [`DataStore`] trait
//!    - Generic over any `StorageBackend`
//!
//! For convenience, type aliases are provided:
//! - [`fs::FileStore`]: `PadStore<FsBackend>` - production use
//! - [`memory::InMemoryStore`]: `PadStore<MemBackend>` - testing
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
use crate::tags::TagEntry;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

pub mod backend;
pub mod bucketed;
pub mod fs;
pub mod fs_backend;
pub mod mem_backend;
pub mod memory;
pub mod pad_store;

/// Which lifecycle bucket a pad lives in.
///
/// Bucket membership IS lifecycle state — there is no separate `is_deleted` flag.
/// Moving a pad between buckets is the mechanism for delete/restore/archive/unarchive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Bucket {
    Active,
    Archived,
    Deleted,
}

/// Report from the `doctor` operation.
#[derive(Debug, Default)]
pub struct DoctorReport {
    pub fixed_missing_files: usize,
    pub recovered_files: usize,
    pub fixed_content_files: usize,
}

/// Abstract interface for pad storage.
///
/// All methods are bucket-aware: pads live in Active, Archived, or Deleted buckets.
/// Tags are scope-level (shared across buckets).
pub trait DataStore {
    /// Save a pad (create or update) in a specific bucket
    fn save_pad(&mut self, pad: &Pad, scope: Scope, bucket: Bucket) -> Result<()>;

    /// Get a pad by ID from a specific bucket
    fn get_pad(&self, id: &Uuid, scope: Scope, bucket: Bucket) -> Result<Pad>;

    /// List all pads in a given scope and bucket
    fn list_pads(&self, scope: Scope, bucket: Bucket) -> Result<Vec<Pad>>;

    /// Delete a pad permanently from a specific bucket
    fn delete_pad(&mut self, id: &Uuid, scope: Scope, bucket: Bucket) -> Result<()>;

    /// Move a pad from one bucket to another.
    /// Returns the pad as it exists in the destination bucket.
    fn move_pad(&mut self, id: &Uuid, scope: Scope, from: Bucket, to: Bucket) -> Result<Pad>;

    /// Move multiple pads between buckets (e.g., parent + descendants).
    fn move_pads(
        &mut self,
        ids: &[Uuid],
        scope: Scope,
        from: Bucket,
        to: Bucket,
    ) -> Result<Vec<Pad>>;

    /// Get the file path for a pad (for file-based stores)
    fn get_pad_path(&self, id: &Uuid, scope: Scope, bucket: Bucket) -> Result<PathBuf>;

    /// Verify and fix consistency issues across all buckets
    fn doctor(&mut self, scope: Scope) -> Result<DoctorReport>;

    // --- Tag Registry Operations (scope-level, not bucket-specific) ---

    /// Load all tags from the registry
    fn load_tags(&self, scope: Scope) -> Result<Vec<TagEntry>>;

    /// Save the tag registry
    fn save_tags(&mut self, scope: Scope, tags: &[TagEntry]) -> Result<()>;
}
