//! # Typed handler results
//!
//! Every padz handler returns one of the types in this module (or `Output::Silent` /
//! `Output::Artifact`). A result is **mode-independent**: the same value is serialized
//! once by standout and then either fed to a MiniJinja template (human modes) or
//! emitted directly (structured modes). Handlers therefore never look at
//! `OutputMode`, never branch on it, and never print.
//!
//! ## What belongs here
//!
//! These are CLI-only adapter types: they exist purely to establish the shell's
//! result contract, so they live in the binary rather than in `padzapp`. The data
//! they carry (`DisplayPad` and semantic mutation/tag/transfer outcomes) is
//! reusable domain data and stays in `padzapp`.
//!
//! ## Presentation is not in here
//!
//! Template-only fields — column widths, glyphs, index strings, relative timestamps,
//! indentation — are **not** part of a result. They are derived at render time by the
//! view builders in [`super::render`], which standout invokes only for human output.
//! That is what keeps structured output free of terminal artifacts while still giving
//! templates everything they need from the very same value.
//!
//! The `request` field on the list/modification results is the exception that proves
//! the rule: it records *what the user asked to see* (peek previews, uuids, status
//! icons), not how to draw it. It is a mode-independent fact about the invocation, so
//! it is part of the result and visible in structured output.

use padzapp::commands::{CmdNotice, CmdOutcome, NestingMode, UpdateKind};
use padzapp::index::{DisplayIndex, DisplayPad};
use padzapp::model::TodoStatus;
use serde::{Deserialize, Serialize};

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

/// The outcome of a command that changed pads.
///
/// `action` is the machine-readable operation token for the change and `pads`
/// are the pads it affected. The human renderer owns the corresponding verb.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModificationResult {
    pub action: ModificationActionResult,
    pub pads: Vec<DisplayPad>,
    /// Machine-readable facts emitted by the core; templates own their prose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<ModificationNoticeResult>,
    /// Machine-readable successful outcomes emitted by the core.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outcomes: Vec<MutationOutcomeResult>,
    pub request: ModificationRequest,
}

/// The operation performed by a generic pad modification command.
///
/// This is a machine-readable token, not the human past-tense verb. The
/// modification template owns wording such as "Pinned" and "Moved".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModificationActionResult {
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

/// CLI projection of a semantic mutation no-op.
///
/// This keeps the shell's structured schema independent from the core enum while
/// retaining the `kind` and canonical display-path shape established for pinning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModificationNoticeResult {
    AlreadyPinned {
        path: Vec<DisplayIndex>,
    },
    AlreadyUnpinned {
        path: Vec<DisplayIndex>,
    },
    AlreadyAtDestination {
        path: Vec<DisplayIndex>,
    },
    AlreadyInStatus {
        path: Vec<DisplayIndex>,
        status: MutationStatusResult,
    },
    NoCompletedPads,
}

impl From<CmdNotice> for ModificationNoticeResult {
    fn from(notice: CmdNotice) -> Self {
        match notice {
            CmdNotice::AlreadyPinned { path } => Self::AlreadyPinned { path },
            CmdNotice::AlreadyUnpinned { path } => Self::AlreadyUnpinned { path },
            CmdNotice::AlreadyAtDestination { path } => Self::AlreadyAtDestination { path },
            CmdNotice::AlreadyInStatus { path, status } => Self::AlreadyInStatus {
                path,
                status: status.into(),
            },
            CmdNotice::NoCompletedPads => Self::NoCompletedPads,
        }
    }
}

/// Requested status exposed by a semantic no-op.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationStatusResult {
    Planned,
    InProgress,
    Done,
}

impl From<TodoStatus> for MutationStatusResult {
    fn from(status: TodoStatus) -> Self {
        match status {
            TodoStatus::Planned => Self::Planned,
            TodoStatus::InProgress => Self::InProgress,
            TodoStatus::Done => Self::Done,
        }
    }
}

/// CLI projection of a successful semantic pad mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MutationOutcomeResult {
    Updated {
        path: Vec<DisplayIndex>,
        title: String,
        update_kind: UpdateKindResult,
    },
    StatusChanged {
        path: Vec<DisplayIndex>,
        status: MutationStatusResult,
    },
}

impl From<CmdOutcome> for MutationOutcomeResult {
    fn from(outcome: CmdOutcome) -> Self {
        match outcome {
            CmdOutcome::Updated {
                path,
                title,
                update_kind,
            } => Self::Updated {
                path,
                title,
                update_kind: update_kind.into(),
            },
            CmdOutcome::StatusChanged { path, status } => Self::StatusChanged {
                path,
                status: status.into(),
            },
        }
    }
}

/// How an update reached the core, as part of the shell's public result schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateKindResult {
    Structured,
    Content,
    Refresh,
}

impl From<UpdateKind> for UpdateKindResult {
    fn from(kind: UpdateKind) -> Self {
        match kind {
            UpdateKind::Structured => Self::Structured,
            UpdateKind::Content => Self::Content,
            UpdateKind::Refresh => Self::Refresh,
        }
    }
}

/// The typed outcome of `create`.
///
/// `Created` deliberately serializes exactly like the existing
/// [`ModificationResult`] so successful-create automation remains compatible.
/// `Aborted` replaces the former prose-only warning with semantic facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CreateResult {
    Created(ModificationResult),
    Aborted(CreateAbortResult),
}

/// Why a create invocation stopped without creating a pad.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateAbortResult {
    pub kind: CreateAbortKindResult,
    pub reason: CreateAbortReasonResult,
}

/// The top-level outcome class for a create abort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreateAbortKindResult {
    Aborted,
}

/// The semantic reason a create invocation aborted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreateAbortReasonResult {
    EmptyContent,
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
