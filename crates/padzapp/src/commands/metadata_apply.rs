//! Defensive per-field metadata application.
//!
//! Both the JSON archive import (`--json`) and the inline metadata import
//! (md frontmatter / lex annotations) need the same logic: apply each metadata
//! field to a `Pad` individually, tolerating unknown or malformed fields and
//! collecting per-field warnings instead of failing the whole import.
//!
//! This module centralizes that logic so PR 1 and PR 2 stay consistent.

use crate::commands::CmdMessage;
use crate::model::{Metadata, Pad, TodoStatus};
use crate::store::Bucket;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use std::collections::HashSet;
use uuid::Uuid;

/// Controls parent_id handling during metadata application.
pub enum ParentPolicy<'a> {
    /// Trust `parent_id` as-is. Use when we have no authoritative set of
    /// known pads (e.g. single md file import with no cross-references).
    Trust,
    /// Orphan the child (`parent_id` → None) if the referenced parent is not
    /// in this set. Used for archives and multi-file imports where we know
    /// which pads are coming along.
    OrphanUnknown(&'a HashSet<Uuid>),
}

/// Apply each known metadata key to `pad`, recording per-field warnings.
///
/// The function is deliberately total: every field is optional and failure
/// to parse a single field never aborts the pad. Use `source_label` (e.g.
/// a file name) as a prefix so warnings are traceable.
///
/// Unknown keys are silently ignored — forward compatibility with future
/// padz versions that may add more metadata.
pub fn apply_metadata_defensively(
    pad: &mut Pad,
    value: &Value,
    parent_policy: ParentPolicy<'_>,
    source_label: &str,
) -> Vec<CmdMessage> {
    let mut warnings = Vec::new();

    let Some(obj) = value.as_object() else {
        warnings.push(CmdMessage::warning(format!(
            "{}: metadata is not an object, keeping defaults",
            source_label
        )));
        return warnings;
    };

    if let Some(id_val) = obj.get("id") {
        match value_to_uuid(id_val) {
            Some(u) => pad.metadata.id = u,
            None => warnings.push(CmdMessage::warning(format!(
                "{}: invalid id field, assigned a new UUID",
                source_label
            ))),
        }
    }

    apply_datetime(
        obj,
        "created_at",
        &mut pad.metadata,
        &mut warnings,
        source_label,
    );
    apply_datetime(
        obj,
        "updated_at",
        &mut pad.metadata,
        &mut warnings,
        source_label,
    );

    if let Some(v) = obj.get("is_pinned") {
        match v.as_bool() {
            Some(b) => pad.metadata.is_pinned = b,
            None => warnings.push(CmdMessage::warning(format!(
                "{}: invalid is_pinned",
                source_label
            ))),
        }
    }
    if let Some(v) = obj.get("pinned_at") {
        if v.is_null() {
            pad.metadata.pinned_at = None;
        } else {
            match value_to_datetime(v) {
                Some(dt) => pad.metadata.pinned_at = Some(dt),
                None => warnings.push(CmdMessage::warning(format!(
                    "{}: invalid pinned_at",
                    source_label
                ))),
            }
        }
    }
    if let Some(v) = obj.get("delete_protected") {
        match v.as_bool() {
            Some(b) => pad.metadata.delete_protected = b,
            None => warnings.push(CmdMessage::warning(format!(
                "{}: invalid delete_protected",
                source_label
            ))),
        }
    }
    if let Some(v) = obj.get("status") {
        match v.as_str().and_then(parse_todo_status) {
            Some(s) => pad.metadata.status = s,
            None => warnings.push(CmdMessage::warning(format!(
                "{}: invalid status",
                source_label
            ))),
        }
    }
    if let Some(v) = obj.get("tags") {
        match v.as_array() {
            Some(arr) => {
                let mut tags = Vec::with_capacity(arr.len());
                let mut bad = 0;
                for t in arr {
                    match t.as_str() {
                        Some(s) => tags.push(s.to_string()),
                        None => bad += 1,
                    }
                }
                pad.metadata.tags = tags;
                if bad > 0 {
                    warnings.push(CmdMessage::warning(format!(
                        "{}: {} non-string tag entries ignored",
                        source_label, bad
                    )));
                }
            }
            None => warnings.push(CmdMessage::warning(format!(
                "{}: invalid tags (not an array)",
                source_label
            ))),
        }
    }

    // Title: metadata title may be truncated; trust the content's first line
    // as truth when it's non-empty, but still allow metadata override when the
    // content couldn't be parsed.
    if let Some(v) = obj.get("title") {
        if let Some(s) = v.as_str() {
            if pad.metadata.title.is_empty() {
                pad.metadata.title = s.to_string();
            }
        }
    }

    if let Some(v) = obj.get("parent_id") {
        apply_parent_id(
            v,
            &mut pad.metadata,
            &parent_policy,
            source_label,
            &mut warnings,
        );
    }

    warnings
}

/// Parse a bucket string. Returns Active on unknown values. The caller may
/// want to emit its own warning instead; we default-tolerantly here to keep
/// the per-field defensive contract.
pub fn parse_bucket_or_active(s: &str) -> Bucket {
    match s {
        "Archived" => Bucket::Archived,
        "Deleted" => Bucket::Deleted,
        _ => Bucket::Active,
    }
}

fn apply_parent_id(
    v: &Value,
    meta: &mut Metadata,
    policy: &ParentPolicy<'_>,
    source_label: &str,
    warnings: &mut Vec<CmdMessage>,
) {
    if v.is_null() {
        meta.parent_id = None;
        return;
    }
    match value_to_uuid(v) {
        Some(pid) => match policy {
            ParentPolicy::Trust => meta.parent_id = Some(pid),
            ParentPolicy::OrphanUnknown(known) => {
                if known.contains(&pid) {
                    meta.parent_id = Some(pid);
                } else {
                    meta.parent_id = None;
                    warnings.push(CmdMessage::info(format!(
                        "{}: parent not in import set, orphaned to root",
                        source_label
                    )));
                }
            }
        },
        None => warnings.push(CmdMessage::warning(format!(
            "{}: invalid parent_id",
            source_label
        ))),
    }
}

fn apply_datetime(
    obj: &Map<String, Value>,
    key: &str,
    meta: &mut Metadata,
    warnings: &mut Vec<CmdMessage>,
    source_label: &str,
) {
    if let Some(v) = obj.get(key) {
        match value_to_datetime(v) {
            Some(dt) => match key {
                "created_at" => meta.created_at = dt,
                "updated_at" => meta.updated_at = dt,
                _ => {}
            },
            None => warnings.push(CmdMessage::warning(format!(
                "{}: invalid {}",
                source_label, key
            ))),
        }
    }
}

fn value_to_uuid(v: &Value) -> Option<Uuid> {
    v.as_str().and_then(|s| Uuid::parse_str(s).ok())
}

fn value_to_datetime(v: &Value) -> Option<DateTime<Utc>> {
    v.as_str()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

fn parse_todo_status(s: &str) -> Option<TodoStatus> {
    match s {
        "Planned" => Some(TodoStatus::Planned),
        "InProgress" => Some(TodoStatus::InProgress),
        "Done" => Some(TodoStatus::Done),
        _ => None,
    }
}
