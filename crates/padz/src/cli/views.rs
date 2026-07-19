//! # CLI-owned thin views
//!
//! padz handlers return `padzapp` **core** types directly wherever possible (e.g.
//! `purge_pads` → `Output<PurgeOutcome>`, `doctor` → `Output<DoctorOutcome>`, and the
//! export/import/transfer and tag families → their core reports). This module holds only
//! the small **thin view** structs kept for the handful of commands where a core shape
//! doesn't fit a template. Every value a handler returns — core type or thin view — is a
//! **mode-independent** value: standout serializes it once and then either feeds it to a
//! MiniJinja template (human modes) or emits it directly (structured modes). Handlers
//! therefore never look at `OutputMode`, never branch on it, and never print.
//!
//! The thin views here follow the same flat shape as the reference `tdoo` example
//! (`TodoListView`, `TodoActionView`, …), and they come in two shapes:
//!
//! - **Pure CLI summaries** — facts that have no `padzapp` core type to return, computed
//!   by the handler itself (resolved filesystem paths, resolved UUID strings, a
//!   clipboard-copy tally). See [`PathView`], [`UuidView`], [`CopyView`].
//! - **Thin views over core types** — a listing or a command outcome that carries
//!   `padzapp` core data ([`DisplayPad`], [`CmdNotice`], [`CmdOutcome`]) **verbatim**,
//!   wrapped with the small CLI-only facts a template needs: the request flags that say
//!   *what to show* and the action token that says *which command ran* (a fact the core
//!   does not model). See [`Listing`], [`Modification`], [`PadContent`].
//!
//! Neither shape is a tier-2 presentation projection, and none round-trips through a
//! render mirror: `padzapp` core serde is untouched, and each value is built once by its
//! handler and serialized once by standout. A template reads their named fields
//! directly; structured modes emit them as-is.
//!
//! ## Presentation is not in here
//!
//! Every field is a fact. No width, glyph, style name, index string, relative timestamp,
//! or rendered sentence — those are presentation policy in `templates/` and
//! `styles/default.css`, derived at render time (the `timeago`/`peek` filters in
//! [`super::render`]) only for human output. The `request` field on the list/modification
//! views is the exception that proves the rule: it records *what the user asked to see*
//! (peek previews, uuids, status icons), not how to draw it — a mode-independent fact
//! about the invocation, so it rides in structured output too.

use padzapp::commands::{CmdNotice, CmdOutcome, NestingMode};
use padzapp::index::DisplayPad;
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

/// What the user asked a listing to show.
///
/// Rides on [`Listing`] and is read by `list.jinja` to decide which columns and
/// previews to draw. Mode-independent: `--peek` means "include previews" whether the
/// output ends up as a table or as JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListRequest {
    /// Show a content preview under each pad (`--peek`, or the `peek` command).
    pub peek: bool,
    /// Prefix titles with a short uuid (`--uuid`).
    pub uuid: bool,
    /// Show todo status icons (todos mode, or `--show-status`).
    pub status: bool,
    /// The listing was narrowed by ids/search/tags/status, so an empty result means
    /// "nothing matched" rather than "no pads yet".
    pub filtered: bool,
    /// Append the deleted-pads help block (`--deleted` / `--archived`).
    pub deleted_help: bool,
    /// Group results under lifecycle section headers (`--all`).
    pub sections: bool,
}

/// What the user asked a modification to show.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModificationRequest {
    /// Show todo status icons (todos mode, or a status-changing command).
    pub status: bool,
}

/// A listing of pads plus what the user asked to show, rendered straight from the
/// core tree.
///
/// Produced by `list`, `peek`, and `search`. `pads` is the core [`DisplayPad`] tree
/// exactly as `index_pads` built it — canonical display identifiers (pinned dual
/// indexes, nested tree indexes) and `children` untouched — and `list.jinja` walks it
/// with a recursive loop rather than a flattened row mirror. `request` records what to
/// show (peek/uuid/status flags), not how to draw it, so it rides in structured output
/// too. Presentation (widths, glyphs, timestamps, the empty-store help) stays in the
/// template and its filters; nothing here is a rendered string.
#[derive(Debug, Clone, Serialize)]
pub struct Listing {
    pub pads: Vec<DisplayPad>,
    pub request: ListRequest,
}

/// The outcome of a command that changed pads, rendered straight from core types.
///
/// This is the thin flat view the modification family returns instead of a tier-2
/// projection: `action` is the machine-readable operation token (the one genuinely
/// CLI-only fact — the core has no "which command ran" enum) and `pads` are the pads
/// it affected. `notices` and `outcomes` are the **core** semantic facts
/// ([`CmdNotice`]/[`CmdOutcome`]) verbatim — they already `derive(Serialize)`, so
/// structured output reads them directly and `modification_result.jinja` owns every
/// verb and sentence. `request` records what to show (status icons), not how to draw.
#[derive(Debug, Clone, Serialize)]
pub struct Modification {
    pub action: ModificationAction,
    pub pads: Vec<DisplayPad>,
    /// Machine-readable facts emitted by the core; templates own their prose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<CmdNotice>,
    /// Machine-readable successful outcomes emitted by the core.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<CmdOutcome>,
    pub request: ModificationRequest,
}

/// The operation performed by a generic pad modification command.
///
/// This is a machine-readable token, not the human past-tense verb, and the one
/// fact the core does not model (it reports *what changed*, not *which command
/// asked*). The modification template owns wording such as "Pinned" and "Moved".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ModificationAction {
    Create,
    Pin,
    Unpin,
    Delete,
    Restore,
    Archive,
    Unarchive,
    Complete,
    Reopen,
    Move,
    Update,
}

/// One pad's full content, as returned by `view`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadContent {
    /// Raw title with no presentation indentation.
    pub title: String,
    /// Raw body (content minus the title line), with no presentation indentation.
    pub content: String,
    /// Depth in the pad tree; 0 for a root pad.
    pub depth: usize,
    /// Present only when `--uuid` was passed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uuid: Option<String>,
}

/// Full content of the viewed pads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadContentResult {
    pub pads: Vec<PadContent>,
    /// The requested relationship shape; human rendering decides how it looks.
    pub nesting: NestingMode,
}
