//! Attribute specifications and registry.
//!
//! This module defines the schema for attributes: what kinds of values they hold,
//! whether they're filterable, and what behaviors they have.

/// The kind of value an attribute holds.
///
/// This defines the type system for attributes, determining how values
/// are stored, compared, and filtered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttributeKind {
    /// Simple boolean (e.g., `delete_protected`)
    Bool,

    /// Boolean with associated timestamp (e.g., `pinned`, `deleted`)
    ///
    /// When the flag is set to true, a timestamp is recorded.
    /// When false, the timestamp is cleared.
    BoolWithTimestamp,

    /// Enum with a fixed set of valid values (e.g., `status`)
    Enum,

    /// List of strings (e.g., `tags`)
    List,

    /// Reference to another pad by UUID (e.g., `parent_id`)
    Ref,
}

/// Specification for a single attribute.
///
/// This struct describes the schema and behavior of an attribute,
/// enabling generic handling across different attribute types.
#[derive(Debug, Clone)]
pub struct AttributeSpec {
    /// The attribute name used in the API (e.g., "pinned", "status", "tags")
    pub name: &'static str,

    /// The kind of value this attribute holds
    pub kind: AttributeKind,

    /// Whether this attribute can be used in list filters (e.g., --status, --tags)
    pub filterable: bool,

    /// Whether setting this attribute should trigger parent status recalculation
    ///
    /// Only applies to `status` - when a child's status changes, the parent's
    /// status may need to be recalculated based on all children.
    pub propagates_up: bool,

    /// Whether deleting this value from a registry cascades to all pads
    ///
    /// Only applies to `tags` - when a tag is deleted from the registry,
    /// it should be removed from all pads that have it.
    pub cascades_on_delete: bool,

    /// Whether this attribute has coupled fields that are set together
    ///
    /// For example, setting `pinned` also sets `delete_protected`.
    /// The coupling logic is implemented in `set_attr()`.
    pub has_coupling: bool,
}

impl AttributeSpec {
    /// Create a new attribute spec with default flags (all false).
    const fn new(name: &'static str, kind: AttributeKind) -> Self {
        Self {
            name,
            kind,
            filterable: false,
            propagates_up: false,
            cascades_on_delete: false,
            has_coupling: false,
        }
    }

    /// Set the filterable flag.
    const fn filterable(mut self) -> Self {
        self.filterable = true;
        self
    }

    /// Set the propagates_up flag.
    const fn propagates_up(mut self) -> Self {
        self.propagates_up = true;
        self
    }

    /// Set the cascades_on_delete flag.
    const fn cascades_on_delete(mut self) -> Self {
        self.cascades_on_delete = true;
        self
    }

    /// Set the has_coupling flag.
    const fn has_coupling(mut self) -> Self {
        self.has_coupling = true;
        self
    }
}

/// Registry of all pad attributes.
///
/// This is the single source of truth for attribute metadata.
/// Adding a new attribute means adding an entry here.
pub const ATTRIBUTES: &[AttributeSpec] = &[
    // Boolean with timestamp attributes
    AttributeSpec::new("pinned", AttributeKind::BoolWithTimestamp)
        .filterable()
        .has_coupling(), // pinned also sets delete_protected
    // Simple boolean attributes
    AttributeSpec::new("protected", AttributeKind::Bool),
    // Enum attributes
    AttributeSpec::new("status", AttributeKind::Enum)
        .filterable()
        .propagates_up(),
    // List attributes
    AttributeSpec::new("tags", AttributeKind::List)
        .filterable()
        .cascades_on_delete(),
    // Reference attributes
    AttributeSpec::new("parent", AttributeKind::Ref),
];

/// Look up an attribute spec by name.
pub fn get_spec(name: &str) -> Option<&'static AttributeSpec> {
    ATTRIBUTES.iter().find(|spec| spec.name == name)
}

/// Get all filterable attribute names.
pub fn filterable_attrs() -> impl Iterator<Item = &'static str> {
    ATTRIBUTES
        .iter()
        .filter(|spec| spec.filterable)
        .map(|spec| spec.name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attributes_registry_has_expected_entries() {
        assert!(get_spec("pinned").is_some());
        assert!(get_spec("protected").is_some());
        assert!(get_spec("status").is_some());
        assert!(get_spec("tags").is_some());
        assert!(get_spec("parent").is_some());
    }

    #[test]
    fn unknown_attribute_returns_none() {
        assert!(get_spec("nonexistent").is_none());
    }

    #[test]
    fn pinned_spec_is_correct() {
        let spec = get_spec("pinned").unwrap();
        assert_eq!(spec.name, "pinned");
        assert_eq!(spec.kind, AttributeKind::BoolWithTimestamp);
        assert!(spec.filterable);
        assert!(spec.has_coupling);
        assert!(!spec.propagates_up);
        assert!(!spec.cascades_on_delete);
    }

    #[test]
    fn status_spec_is_correct() {
        let spec = get_spec("status").unwrap();
        assert_eq!(spec.name, "status");
        assert_eq!(spec.kind, AttributeKind::Enum);
        assert!(spec.filterable);
        assert!(spec.propagates_up);
    }

    #[test]
    fn tags_spec_is_correct() {
        let spec = get_spec("tags").unwrap();
        assert_eq!(spec.name, "tags");
        assert_eq!(spec.kind, AttributeKind::List);
        assert!(spec.filterable);
        assert!(spec.cascades_on_delete);
    }

    #[test]
    fn protected_spec_is_correct() {
        let spec = get_spec("protected").unwrap();
        assert_eq!(spec.name, "protected");
        assert_eq!(spec.kind, AttributeKind::Bool);
        assert!(!spec.filterable); // internal, not user-filterable
    }

    #[test]
    fn parent_spec_is_correct() {
        let spec = get_spec("parent").unwrap();
        assert_eq!(spec.name, "parent");
        assert_eq!(spec.kind, AttributeKind::Ref);
        assert!(!spec.filterable); // not user-filterable (hierarchy is structural)
    }

    #[test]
    fn filterable_attrs_returns_expected() {
        let filterable: Vec<_> = filterable_attrs().collect();
        assert!(filterable.contains(&"pinned"));
        assert!(filterable.contains(&"status"));
        assert!(filterable.contains(&"tags"));
        assert!(!filterable.contains(&"protected"));
        assert!(!filterable.contains(&"parent"));
    }
}
