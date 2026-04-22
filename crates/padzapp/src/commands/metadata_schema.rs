//! Schema for the `--json` export/import format.
//!
//! The JSON archive is a `.tar.gz` containing:
//! - `padz/db.json` — this schema, with per-pad metadata and the referenced tag registry
//! - `padz/pads/pad-<uuid>.<ext>` — raw pad files, preserving original extension
//!
//! ## Versioning
//!
//! [`Archive::schema_version`] is the only required forward-compat hook.
//! Metadata fields are deserialized defensively: unknown fields are tolerated
//! and missing/malformed fields are skipped on import (pad + file always land;
//! individual metadata errors become warnings).
//!
//! This means the `Metadata` in [`PadEntry::metadata`] is a `serde_json::Value`,
//! not a `Metadata` struct — so field-level import failures don't poison the
//! whole pad. The importer walks the value field-by-field.
//!
//! ## Parent orphaning
//!
//! On export, parent_ids are preserved verbatim. On import, if a pad's
//! `parent_id` is not present in the archive, the parent is set to `None`
//! (see spec: "hierarchy can only be preserved on the full tree being moved").
//! This orphaning happens in the import path, not here.
//!
//! ## Tags
//!
//! Only tag registry entries referenced by exported pads are included —
//! the archive does not export the full scope's tag registry.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Current schema version. Increment on incompatible changes.
pub const SCHEMA_VERSION: u32 = 1;

/// Top-level archive descriptor written as `padz/db.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Archive {
    pub schema_version: u32,
    pub exported_at: DateTime<Utc>,
    pub padz_version: String,
    pub pads: Vec<PadEntry>,
    #[serde(default)]
    pub tags: Vec<TagRegistryEntry>,
}

/// A single pad in the archive: pointer to its file + raw metadata blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadEntry {
    /// Relative path inside the archive (e.g. "pads/pad-<uuid>.lex").
    pub file: String,
    /// Which bucket the pad lived in at export time. Defaults to `"Active"`
    /// on import if missing.
    #[serde(default = "default_bucket")]
    pub bucket: String,
    /// Raw metadata as `serde_json::Value` to allow field-level defensive import.
    pub metadata: Value,
}

fn default_bucket() -> String {
    "Active".to_string()
}

/// Tag registry entry. Kept independent of `tags::TagEntry` so schema
/// evolution in the app doesn't silently break the archive contract.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRegistryEntry {
    pub name: String,
    pub created_at: DateTime<Utc>,
}
