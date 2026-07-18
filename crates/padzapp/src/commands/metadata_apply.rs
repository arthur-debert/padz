//! Thin import-boundary adapter over [`crate::model::Metadata::apply_json_patch`].
//!
//! The defensive "apply every field, collect per-field outcomes" logic lives
//! on the [`crate::model::Metadata`] model. This module adds the source label
//! and stable semantic categories needed by import clients. It deliberately
//! returns no authored prose: a CLI can render compatible diagnostics while a
//! structured client branches on category, reason, field, count, and severity.

use crate::model::{MetadataPatchWarning, Pad};
use crate::store::Bucket;
use serde::{Deserialize, Serialize};

pub use crate::model::ParentPolicy;

/// Severity of a recoverable metadata-application outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataWarningSeverity {
    Info,
    Warning,
}

/// Stable area of metadata affected by a warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataWarningCategory {
    Metadata,
    Field,
    Tags,
    Parent,
}

/// Stable reason a metadata value was not applied as requested.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MetadataWarningReason {
    NotAnObject,
    InvalidValue,
    NonStringEntries,
    OutsideImportSet,
}

/// One typed, source-labelled metadata application warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetadataApplicationWarning {
    pub source_label: String,
    pub category: MetadataWarningCategory,
    pub reason: MetadataWarningReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<usize>,
    pub severity: MetadataWarningSeverity,
}

/// Apply `value` to `pad.metadata` defensively and return stable warning facts.
pub fn apply_metadata_defensively(
    pad: &mut Pad,
    value: &serde_json::Value,
    parent_policy: ParentPolicy<'_>,
    source_label: &str,
) -> Vec<MetadataApplicationWarning> {
    pad.metadata
        .apply_json_patch(value, &parent_policy)
        .into_iter()
        .map(|warning| warning_fact(warning, source_label))
        .collect()
}

fn warning_fact(warning: MetadataPatchWarning, source_label: &str) -> MetadataApplicationWarning {
    use MetadataPatchWarning as Warning;

    let (category, reason, field, count) = match warning {
        Warning::NotAnObject => (
            MetadataWarningCategory::Metadata,
            MetadataWarningReason::NotAnObject,
            None,
            None,
        ),
        Warning::InvalidId => (
            MetadataWarningCategory::Field,
            MetadataWarningReason::InvalidValue,
            Some("id".to_string()),
            None,
        ),
        Warning::InvalidField(field) => (
            MetadataWarningCategory::Field,
            MetadataWarningReason::InvalidValue,
            Some(field.to_string()),
            None,
        ),
        Warning::NonStringTags(count) => (
            MetadataWarningCategory::Tags,
            MetadataWarningReason::NonStringEntries,
            Some("tags".to_string()),
            Some(count),
        ),
        Warning::ParentOrphaned => (
            MetadataWarningCategory::Parent,
            MetadataWarningReason::OutsideImportSet,
            Some("parent_id".to_string()),
            None,
        ),
    };

    MetadataApplicationWarning {
        source_label: source_label.to_string(),
        category,
        reason,
        field,
        count,
        severity: if warning.is_info() {
            MetadataWarningSeverity::Info
        } else {
            MetadataWarningSeverity::Warning
        },
    }
}

/// Parse a bucket label from the on-wire archive format. Unknown labels map
/// to [`Bucket::Active`] — callers that want to warn on unknown buckets
/// should check before calling.
pub fn parse_bucket_or_active(s: &str) -> Bucket {
    match s {
        "Archived" => Bucket::Archived,
        "Deleted" => Bucket::Deleted,
        _ => Bucket::Active,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pad;

    #[test]
    fn warning_facts_keep_source_category_reason_field_count_and_severity() {
        let mut pad = Pad::new("Title".into(), "Body".into());
        let warnings = apply_metadata_defensively(
            &mut pad,
            &serde_json::json!({"status": "broken", "tags": ["ok", 7]}),
            ParentPolicy::Trust,
            "entry.lex",
        );

        assert_eq!(warnings.len(), 2);
        assert_eq!(warnings[0].source_label, "entry.lex");
        assert_eq!(warnings[0].category, MetadataWarningCategory::Field);
        assert_eq!(warnings[0].reason, MetadataWarningReason::InvalidValue);
        assert_eq!(warnings[0].field.as_deref(), Some("status"));
        assert_eq!(warnings[0].severity, MetadataWarningSeverity::Warning);
        assert_eq!(warnings[1].category, MetadataWarningCategory::Tags);
        assert_eq!(warnings[1].count, Some(1));
    }
}
