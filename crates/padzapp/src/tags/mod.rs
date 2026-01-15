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

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub use validation::{validate_tag_name, TagValidationError};

/// A tag entry in the tag registry.
///
/// Tags are stored in a registry and must be created before they can be
/// assigned to pads. This struct represents a single tag in that registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagEntry {
    /// The tag name (validated according to tag naming rules)
    pub name: String,
    /// When this tag was created
    pub created_at: DateTime<Utc>,
}

impl TagEntry {
    /// Creates a new tag entry with the given name.
    ///
    /// Note: This does not validate the tag name. Use [`validate_tag_name`]
    /// before creating a TagEntry to ensure the name is valid.
    pub fn new(name: String) -> Self {
        Self {
            name,
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tag_entry_new() {
        let tag = TagEntry::new("work".to_string());
        assert_eq!(tag.name, "work");
    }

    #[test]
    fn test_tag_entry_serialization_roundtrip() {
        let tag = TagEntry::new("my-project".to_string());
        let json = serde_json::to_string(&tag).unwrap();
        let loaded: TagEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.name, "my-project");
        assert_eq!(loaded.created_at, tag.created_at);
    }
}
