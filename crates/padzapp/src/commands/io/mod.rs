//! Import/export of pads to and from archive files and single-file formats.
//!
//! This module groups the two file-IO commands together because they share
//! archive schema, inline-metadata serialization, and roundtrip invariants.
//!
//! For moving pads *between stores* (clone/migrate), see [`crate::commands::transfer`].

pub mod export;
pub mod import;
