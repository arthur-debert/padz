//! # CLI-owned thin views
//!
//! A handful of commands report facts that have **no `padzapp` core type** to return:
//! they are CLI-level summaries computed by the handler itself (a list of resolved
//! filesystem paths, a list of resolved UUID strings, a clipboard-copy tally). They are
//! not projections of a domain outcome and they are not round-tripped through a render
//! mirror — each is built once by its handler and serialized once by standout, which is
//! exactly the thin flat view shape the reference `tdoo` example uses (`TodoListView`,
//! `TodoActionView`, …).
//!
//! They live here, in the binary, rather than in `super::result` (the doomed
//! presentation-firewall tier) so that removing that tier leaves them untouched. A
//! template reads their named fields directly; structured modes emit them as-is.
//!
//! Every field is a fact. No width, glyph, style name, or rendered sentence — those are
//! presentation policy in `templates/` and `styles/default.css`.

use serde::{Deserialize, Serialize};

/// Filesystem paths of the selected pads, one per selector match (`path` command).
///
/// A named-field wrapper because a template renders a top-level map, not a bare
/// sequence; `path.jinja` iterates `paths`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PathView {
    pub paths: Vec<String>,
}

/// UUIDs of the selected pads, one per selector match (`uuid` command).
///
/// Canonical UUID strings in selector order, with ranges in display order.
/// A named-field wrapper for the same reason as [`PathView`]; `uuid.jinja` iterates
/// `uuids`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UuidView {
    pub uuids: Vec<String>,
}

/// Facts reported after copying pads to the system clipboard (`copy`/`cp` command).
///
/// Only selected roots contribute to the count and titles. Descendants remain in the
/// clipboard payload according to the requested nesting mode, but they are not
/// additional user selections. The clipboard write itself is a handler side effect;
/// this view carries only what the confirmation line reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CopyView {
    pub root_pad_count: usize,
    pub titles: Vec<String>,
}
