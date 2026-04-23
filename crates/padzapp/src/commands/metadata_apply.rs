//! Thin CLI-layer adapter over [`Metadata::apply_json_patch`].
//!
//! The defensive "apply every field, collect per-field warnings" logic lives
//! on the `Metadata` model — see [`crate::model::Metadata::apply_json_patch`].
//! This module's job is only:
//!
//! - convert model-level [`MetadataPatchWarning`]s into user-facing
//!   [`CmdMessage`]s, tagging each with the source (file name / pad id)
//! - parse bucket labels coming off the wire (a store concept, not a model one)
//!
//! The JSON-archive import and the md/lex inline-metadata import both go
//! through this adapter so warning formatting stays consistent.

use crate::commands::CmdMessage;
use crate::model::{MetadataPatchWarning, Pad};
use crate::store::Bucket;

pub use crate::model::ParentPolicy;

/// Apply `value` to `pad.metadata` defensively and return user-facing
/// messages prefixed with `source_label` (e.g. a file path or pad id).
pub fn apply_metadata_defensively(
    pad: &mut Pad,
    value: &serde_json::Value,
    parent_policy: ParentPolicy<'_>,
    source_label: &str,
) -> Vec<CmdMessage> {
    pad.metadata
        .apply_json_patch(value, &parent_policy)
        .into_iter()
        .map(|w| warning_to_message(w, source_label))
        .collect()
}

fn warning_to_message(w: MetadataPatchWarning, source_label: &str) -> CmdMessage {
    let text = format!("{}: {}", source_label, w);
    if w.is_info() {
        CmdMessage::info(text)
    } else {
        CmdMessage::warning(text)
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
