//! Attribute value types and side effects.
//!
//! This module defines the runtime representation of attribute values
//! and the side effects that can result from setting an attribute.

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Runtime representation of an attribute value.
///
/// This enum captures all possible value types that attributes can hold.
/// It's used for both getting and setting attributes through the unified API.
#[derive(Debug, Clone, PartialEq)]
pub enum AttrValue {
    /// Simple boolean value (e.g., `delete_protected`)
    Bool(bool),

    /// Boolean with associated timestamp (e.g., `pinned`, `deleted`)
    ///
    /// The timestamp is `Some` when the flag is true, `None` when false.
    BoolWithTimestamp {
        value: bool,
        timestamp: Option<DateTime<Utc>>,
    },

    /// Enum value as string (e.g., `status` = "Planned" | "InProgress" | "Done")
    Enum(String),

    /// List of strings (e.g., `tags`)
    List(Vec<String>),

    /// Optional reference to another pad (e.g., `parent_id`)
    Ref(Option<Uuid>),
}

impl AttrValue {
    /// Create a BoolWithTimestamp value.
    ///
    /// If `value` is true, sets timestamp to now. If false, timestamp is None.
    pub fn bool_with_timestamp(value: bool) -> Self {
        AttrValue::BoolWithTimestamp {
            value,
            timestamp: if value { Some(Utc::now()) } else { None },
        }
    }

    /// Check if this value represents a "truthy" state for filtering.
    ///
    /// - Bool: the boolean value itself
    /// - BoolWithTimestamp: the boolean value
    /// - Enum: always true (has a value)
    /// - List: true if non-empty
    /// - Ref: true if Some
    pub fn is_truthy(&self) -> bool {
        match self {
            AttrValue::Bool(v) => *v,
            AttrValue::BoolWithTimestamp { value, .. } => *value,
            AttrValue::Enum(_) => true,
            AttrValue::List(v) => !v.is_empty(),
            AttrValue::Ref(v) => v.is_some(),
        }
    }

    /// Get the boolean value if this is a Bool or BoolWithTimestamp.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AttrValue::Bool(v) => Some(*v),
            AttrValue::BoolWithTimestamp { value, .. } => Some(*value),
            _ => None,
        }
    }

    /// Get the string value if this is an Enum.
    pub fn as_enum(&self) -> Option<&str> {
        match self {
            AttrValue::Enum(s) => Some(s),
            _ => None,
        }
    }

    /// Get the list if this is a List.
    pub fn as_list(&self) -> Option<&[String]> {
        match self {
            AttrValue::List(v) => Some(v),
            _ => None,
        }
    }

    /// Get the UUID if this is a Ref.
    pub fn as_ref(&self) -> Option<Option<Uuid>> {
        match self {
            AttrValue::Ref(v) => Some(*v),
            _ => None,
        }
    }
}

/// Side effects that result from setting an attribute.
///
/// When `set_attr()` modifies a metadata field, it may trigger additional
/// actions that the caller needs to handle. This enum signals what those are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrSideEffect {
    /// No additional action needed.
    None,

    /// Status changed; parent status should be recalculated.
    ///
    /// The caller should call `propagate_status_change()` with the parent_id.
    PropagateStatusUp,

    /// Tags were modified; may need registry validation.
    ///
    /// Contains the tag names that were added (caller should validate they exist).
    ValidateTags(Vec<String>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_with_timestamp_true_has_timestamp() {
        let value = AttrValue::bool_with_timestamp(true);
        match value {
            AttrValue::BoolWithTimestamp { value, timestamp } => {
                assert!(value);
                assert!(timestamp.is_some());
            }
            _ => panic!("Expected BoolWithTimestamp"),
        }
    }

    #[test]
    fn bool_with_timestamp_false_has_no_timestamp() {
        let value = AttrValue::bool_with_timestamp(false);
        match value {
            AttrValue::BoolWithTimestamp { value, timestamp } => {
                assert!(!value);
                assert!(timestamp.is_none());
            }
            _ => panic!("Expected BoolWithTimestamp"),
        }
    }

    #[test]
    fn is_truthy_for_bool() {
        assert!(AttrValue::Bool(true).is_truthy());
        assert!(!AttrValue::Bool(false).is_truthy());
    }

    #[test]
    fn is_truthy_for_bool_with_timestamp() {
        assert!(AttrValue::bool_with_timestamp(true).is_truthy());
        assert!(!AttrValue::bool_with_timestamp(false).is_truthy());
    }

    #[test]
    fn is_truthy_for_list() {
        assert!(AttrValue::List(vec!["a".into()]).is_truthy());
        assert!(!AttrValue::List(vec![]).is_truthy());
    }

    #[test]
    fn is_truthy_for_ref() {
        assert!(AttrValue::Ref(Some(Uuid::new_v4())).is_truthy());
        assert!(!AttrValue::Ref(None).is_truthy());
    }

    #[test]
    fn is_truthy_for_enum() {
        // Enums are always truthy (they always have a value)
        assert!(AttrValue::Enum("Done".into()).is_truthy());
        assert!(AttrValue::Enum("".into()).is_truthy());
    }

    #[test]
    fn as_bool_extracts_boolean() {
        assert_eq!(AttrValue::Bool(true).as_bool(), Some(true));
        assert_eq!(AttrValue::Bool(false).as_bool(), Some(false));
        assert_eq!(AttrValue::bool_with_timestamp(true).as_bool(), Some(true));
        assert_eq!(AttrValue::Enum("x".into()).as_bool(), None);
    }

    #[test]
    fn as_enum_extracts_string() {
        assert_eq!(AttrValue::Enum("Done".into()).as_enum(), Some("Done"));
        assert_eq!(AttrValue::Bool(true).as_enum(), None);
    }

    #[test]
    fn as_list_extracts_vec() {
        let list = vec!["a".to_string(), "b".to_string()];
        assert_eq!(
            AttrValue::List(list.clone()).as_list(),
            Some(list.as_slice())
        );
        assert_eq!(AttrValue::Bool(true).as_list(), None);
    }

    #[test]
    fn as_ref_extracts_uuid() {
        let id = Uuid::new_v4();
        assert_eq!(AttrValue::Ref(Some(id)).as_ref(), Some(Some(id)));
        assert_eq!(AttrValue::Ref(None).as_ref(), Some(None));
        assert_eq!(AttrValue::Bool(true).as_ref(), None);
    }
}
