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
//! they carry (`DisplayPad`, `CmdMessage`, `PeekResult`) is reusable domain data and
//! stays in `padzapp`.
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

use padzapp::api::CmdMessage;
use padzapp::commands::{CmdNotice, NestingMode};
use padzapp::index::DisplayPad;
use serde::{Deserialize, Serialize};

/// What the user asked a listing to show.
///
/// Consumed by [`super::render::build_list_view`] to decide which columns and
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

/// Pads matching a listing, in canonical display order.
///
/// Produced by `list`, `peek`, and `search`. `pads` keeps the canonical display
/// identifiers assigned by `index_pads` — including pinned dual indexes and nested
/// tree indexes — untouched.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PadListResult {
    pub pads: Vec<DisplayPad>,
    pub messages: Vec<CmdMessage>,
    pub request: ListRequest,
}

/// The outcome of a command that changed pads.
///
/// `action` is the past-tense verb for the change ("Pinned", "Deleted", ...) and
/// `pads` are the pads it affected.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModificationResult {
    pub action: String,
    pub pads: Vec<DisplayPad>,
    pub messages: Vec<CmdMessage>,
    /// Machine-readable facts emitted by the core; templates own their prose.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notices: Vec<CmdNotice>,
    pub request: ModificationRequest,
}

/// A command whose whole result is user-facing messages.
///
/// Rendered by `messages.jinja`, which reads `CmdMessage` directly — no view
/// derivation, so no view builder is registered for this shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagesResult {
    pub messages: Vec<CmdMessage>,
}

impl MessagesResult {
    pub fn new(messages: Vec<CmdMessage>) -> Self {
        Self { messages }
    }
}

/// The machine-readable report carried by an export artifact.
///
/// Standout wraps this value with its own `receipt` after the final write. An
/// empty selection renders this value directly with `status = "empty"` and
/// never enters the artifact path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportReportResult {
    pub status: ExportStatus,
    pub format: ExportFormat,
    pub exported: usize,
    pub warnings: Vec<ExportWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportStatus {
    Empty,
    Exported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    Archive,
    MetadataArchive,
    JsonArchive,
    SingleFile,
}

/// A CLI-owned projection of a semantic core warning.
///
/// The template owns the sentence; structured modes retain the warning kind,
/// complete title list, count, preview, and overflow count as facts.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExportWarning {
    MetadataUnavailable {
        count: usize,
        titles: Vec<String>,
        preview: Vec<String>,
        additional: usize,
    },
}

impl ExportReportResult {
    pub fn empty(format: padzapp::commands::export::ExportFormat) -> Self {
        Self {
            status: ExportStatus::Empty,
            format: format.into(),
            exported: 0,
            warnings: Vec::new(),
        }
    }
}

impl From<padzapp::commands::export::ExportReport> for ExportReportResult {
    fn from(report: padzapp::commands::export::ExportReport) -> Self {
        Self {
            status: ExportStatus::Exported,
            format: report.format.into(),
            exported: report.exported,
            warnings: report.warnings.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<padzapp::commands::export::ExportFormat> for ExportFormat {
    fn from(format: padzapp::commands::export::ExportFormat) -> Self {
        match format {
            padzapp::commands::export::ExportFormat::Archive => Self::Archive,
            padzapp::commands::export::ExportFormat::MetadataArchive => Self::MetadataArchive,
            padzapp::commands::export::ExportFormat::JsonArchive => Self::JsonArchive,
            padzapp::commands::export::ExportFormat::SingleFile => Self::SingleFile,
        }
    }
}

impl From<padzapp::commands::export::ExportWarning> for ExportWarning {
    fn from(warning: padzapp::commands::export::ExportWarning) -> Self {
        match warning {
            padzapp::commands::export::ExportWarning::MetadataUnavailable { titles } => {
                let count = titles.len();
                let preview = titles.iter().take(3).cloned().collect();
                Self::MetadataUnavailable {
                    count,
                    titles,
                    preview,
                    additional: count.saturating_sub(3),
                }
            }
        }
    }
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

/// Filesystem paths of the selected pads, one per selector match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathResult {
    pub paths: Vec<String>,
}

/// UUIDs of the selected pads, one per selector match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UuidResult {
    pub uuids: Vec<String>,
}
