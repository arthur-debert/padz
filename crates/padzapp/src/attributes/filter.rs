//! Attribute filtering.
//!
//! This module provides a unified way to filter pads based on attribute values.
//! Instead of separate filter functions for each attribute type, `AttrFilter`
//! expresses filter conditions that can be applied to any pad.

use super::AttrValue;
use crate::model::Metadata;

/// Filter operation for comparing attribute values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterOp {
    /// Exact equality match.
    Eq,
    /// Not equal.
    Ne,
    /// List contains the specified value (for List attributes).
    Contains,
    /// List contains ALL specified values (for List attributes, AND logic).
    ContainsAll,
}

/// A filter condition on an attribute.
///
/// Combines an attribute name, an operation, and a value to match against.
#[derive(Debug, Clone)]
pub struct AttrFilter {
    /// The attribute name (e.g., "pinned", "status", "tags")
    pub attr: String,
    /// The filter operation
    pub op: FilterOp,
    /// The value to compare against
    pub value: AttrValue,
}

impl AttrFilter {
    /// Create a new filter condition.
    pub fn new(attr: impl Into<String>, op: FilterOp, value: AttrValue) -> Self {
        Self {
            attr: attr.into(),
            op,
            value,
        }
    }

    /// Convenience: create an equality filter.
    pub fn eq(attr: impl Into<String>, value: AttrValue) -> Self {
        Self::new(attr, FilterOp::Eq, value)
    }

    /// Convenience: create a not-equal filter.
    pub fn ne(attr: impl Into<String>, value: AttrValue) -> Self {
        Self::new(attr, FilterOp::Ne, value)
    }

    /// Convenience: create a contains filter for lists.
    pub fn contains(attr: impl Into<String>, value: String) -> Self {
        Self::new(attr, FilterOp::Contains, AttrValue::List(vec![value]))
    }

    /// Convenience: create a contains-all filter for lists.
    pub fn contains_all(attr: impl Into<String>, values: Vec<String>) -> Self {
        Self::new(attr, FilterOp::ContainsAll, AttrValue::List(values))
    }

    /// Check if this filter matches the given metadata.
    ///
    /// Returns `true` if the metadata's attribute value satisfies the filter condition.
    /// Returns `false` if the attribute doesn't exist or doesn't match.
    pub fn matches(&self, meta: &Metadata) -> bool {
        let Some(attr_value) = meta.get_attr(&self.attr) else {
            return false;
        };

        match &self.op {
            FilterOp::Eq => self.values_equal(&attr_value, &self.value),
            FilterOp::Ne => !self.values_equal(&attr_value, &self.value),
            FilterOp::Contains => self.list_contains(&attr_value, &self.value),
            FilterOp::ContainsAll => self.list_contains_all(&attr_value, &self.value),
        }
    }

    /// Check if two attribute values are equal.
    fn values_equal(&self, a: &AttrValue, b: &AttrValue) -> bool {
        match (a, b) {
            (AttrValue::Bool(a_val), AttrValue::Bool(b_val)) => a_val == b_val,
            (
                AttrValue::BoolWithTimestamp { value: a_val, .. },
                AttrValue::BoolWithTimestamp { value: b_val, .. },
            ) => a_val == b_val,
            // Allow comparing BoolWithTimestamp with Bool (check the boolean value)
            (AttrValue::BoolWithTimestamp { value: a_val, .. }, AttrValue::Bool(b_val)) => {
                a_val == b_val
            }
            (AttrValue::Bool(a_val), AttrValue::BoolWithTimestamp { value: b_val, .. }) => {
                a_val == b_val
            }
            (AttrValue::Enum(a_val), AttrValue::Enum(b_val)) => a_val == b_val,
            (AttrValue::List(a_list), AttrValue::List(b_list)) => a_list == b_list,
            (AttrValue::Ref(a_ref), AttrValue::Ref(b_ref)) => a_ref == b_ref,
            _ => false, // Different types are not equal
        }
    }

    /// Check if a list attribute contains the specified value(s).
    ///
    /// For Contains: checks if the list has ANY of the specified values.
    fn list_contains(&self, attr_value: &AttrValue, filter_value: &AttrValue) -> bool {
        let AttrValue::List(attr_list) = attr_value else {
            return false;
        };
        let AttrValue::List(filter_list) = filter_value else {
            return false;
        };

        // Contains: attribute list has at least one of the filter values
        filter_list.iter().any(|v| attr_list.contains(v))
    }

    /// Check if a list attribute contains ALL the specified values.
    fn list_contains_all(&self, attr_value: &AttrValue, filter_value: &AttrValue) -> bool {
        let AttrValue::List(attr_list) = attr_value else {
            return false;
        };
        let AttrValue::List(filter_list) = filter_value else {
            return false;
        };

        // ContainsAll: attribute list has ALL of the filter values (AND logic)
        filter_list.iter().all(|v| attr_list.contains(v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TodoStatus;

    fn meta_with_status(status: TodoStatus) -> Metadata {
        let mut meta = Metadata::new("Test".into());
        meta.status = status;
        meta
    }

    fn meta_with_tags(tags: Vec<&str>) -> Metadata {
        let mut meta = Metadata::new("Test".into());
        meta.tags = tags.into_iter().map(|s| s.to_string()).collect();
        meta
    }

    fn meta_pinned() -> Metadata {
        let mut meta = Metadata::new("Test".into());
        meta.is_pinned = true;
        meta.pinned_at = Some(chrono::Utc::now());
        meta
    }

    #[test]
    fn filter_eq_bool() {
        let filter = AttrFilter::eq("pinned", AttrValue::Bool(true));

        assert!(filter.matches(&meta_pinned()));
        assert!(!filter.matches(&Metadata::new("Test".into())));
    }

    #[test]
    fn filter_ne_bool() {
        let filter = AttrFilter::ne("pinned", AttrValue::Bool(true));

        assert!(!filter.matches(&meta_pinned()));
        assert!(filter.matches(&Metadata::new("Test".into())));
    }

    #[test]
    fn filter_eq_status() {
        let filter = AttrFilter::eq("status", AttrValue::Enum("Done".into()));

        assert!(filter.matches(&meta_with_status(TodoStatus::Done)));
        assert!(!filter.matches(&meta_with_status(TodoStatus::Planned)));
        assert!(!filter.matches(&meta_with_status(TodoStatus::InProgress)));
    }

    #[test]
    fn filter_ne_status() {
        let filter = AttrFilter::ne("status", AttrValue::Enum("Done".into()));

        assert!(!filter.matches(&meta_with_status(TodoStatus::Done)));
        assert!(filter.matches(&meta_with_status(TodoStatus::Planned)));
        assert!(filter.matches(&meta_with_status(TodoStatus::InProgress)));
    }

    #[test]
    fn filter_contains_single_tag() {
        let filter = AttrFilter::contains("tags", "rust".into());

        assert!(filter.matches(&meta_with_tags(vec!["rust"])));
        assert!(filter.matches(&meta_with_tags(vec!["rust", "work"])));
        assert!(!filter.matches(&meta_with_tags(vec!["python"])));
        assert!(!filter.matches(&meta_with_tags(vec![])));
    }

    #[test]
    fn filter_contains_all_tags() {
        let filter = AttrFilter::contains_all("tags", vec!["rust".into(), "work".into()]);

        assert!(filter.matches(&meta_with_tags(vec!["rust", "work"])));
        assert!(filter.matches(&meta_with_tags(vec!["rust", "work", "urgent"])));
        assert!(!filter.matches(&meta_with_tags(vec!["rust"])));
        assert!(!filter.matches(&meta_with_tags(vec!["work"])));
        assert!(!filter.matches(&meta_with_tags(vec![])));
    }

    #[test]
    fn filter_unknown_attr_returns_false() {
        let filter = AttrFilter::eq("unknown", AttrValue::Bool(true));
        assert!(!filter.matches(&Metadata::new("Test".into())));
    }

    #[test]
    fn filter_type_mismatch_returns_false() {
        // Trying to compare status (Enum) with a Bool
        let filter = AttrFilter::eq("status", AttrValue::Bool(true));
        assert!(!filter.matches(&meta_with_status(TodoStatus::Done)));
    }

    #[test]
    fn filter_deleted() {
        let filter = AttrFilter::eq("deleted", AttrValue::Bool(true));

        let mut deleted_meta = Metadata::new("Test".into());
        deleted_meta.is_deleted = true;
        deleted_meta.deleted_at = Some(chrono::Utc::now());

        assert!(filter.matches(&deleted_meta));
        assert!(!filter.matches(&Metadata::new("Test".into())));
    }
}
