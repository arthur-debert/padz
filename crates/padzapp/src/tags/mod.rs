//! Tag support for padz.
//!
//! Tags provide a way to categorize and filter pads. Each scope (project/global)
//! maintains its own tag registry, and tags must be explicitly created before
//! they can be assigned to pads.
//!
//! ## Tag Registry
//!
//! Tags are stored in a registry within each scope's `data.json`. Before a tag
//! can be assigned to a pad, it must exist in the registry. This ensures:
//! - Consistent tag naming across pads
//! - Ability to rename tags (updates all pads)
//! - Clean deletion (removes from all pads)
//!
//! ## Tag Naming Rules
//!
//! See [`validation`] module for the full rules. In summary:
//! - Alphanumeric, underscore, and hyphen only
//! - Must start with a letter
//! - No consecutive hyphens, cannot end with hyphen

pub mod validation;

pub use validation::{validate_tag_name, TagValidationError};
