use std::fmt;
use thiserror::Error;
use uuid::Uuid;

/// One pad that matched an ambiguous title selector.
///
/// Carries the *data* a presenter needs — the pad's display index and its
/// title — with no formatting applied. The CLI styles these (accenting the
/// index, highlighting the matched substring in the title); other consumers
/// can render them however they like, or ignore them and use the counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AmbiguityCandidate {
    /// The pad's display index, formatted as the user would type it
    /// (`1`, `p2`, `3.1`, `d4`).
    pub index: String,
    /// The pad's title, verbatim.
    pub title: String,
}

/// Render [`PadzError::AmbiguousTitle`] as plain, unstyled text.
///
/// This is the library's fallback presentation: correct and readable, but
/// with no terminal styling. The padz CLI intercepts the variant before it
/// reaches `Display` and renders a styled equivalent from the same fields.
fn plain_ambiguity(term: &str, total: usize, candidates: &[AmbiguityCandidate]) -> String {
    if candidates.is_empty() {
        return format!(
            "Term \"{}\" matches {} pads. Please be more specific.",
            term, total
        );
    }
    let listing = candidates
        .iter()
        .map(|c| format!("    {}. {}", c.index, c.title))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Term \"{}\" matches multiple pads. Use one, or be more specific:\n{}",
        term, listing
    )
}

#[derive(Error, Debug)]
pub enum PadzError {
    #[error("Pad not found: {0}")]
    PadNotFound(Uuid),

    /// A title selector matched more than one pad.
    ///
    /// `candidates` is populated when the match count is small enough to
    /// enumerate helpfully, and empty when it is not — in which case `total`
    /// is the only useful signal. `total` is always the full match count, so
    /// `candidates.len()` may be less than `total` only when empty.
    #[error("{}", plain_ambiguity(.term, *.total, .candidates))]
    AmbiguousTitle {
        term: String,
        total: usize,
        candidates: Vec<AmbiguityCandidate>,
    },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Store error: {0}")]
    Store(String),

    #[error("Api Error: {0}")]
    Api(String),
}

/// A non-fatal condition raised while initializing a padz context.
///
/// Initialization is best-effort about self-healing work like layout
/// migration: a failure there does not stop the command, but the user should
/// hear about it. The library returns these as data on [`crate::init::PadzContext`]
/// rather than writing to stderr, leaving the decision of whether and how to
/// surface them to the application.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitWarning {
    /// A legacy flat layout was detected at `path` but could not be migrated
    /// to the bucketed layout. The store is left as-is and remains readable.
    MigrationFailed {
        path: std::path::PathBuf,
        error: String,
    },
}

impl fmt::Display for InitWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InitWarning::MigrationFailed { path, error } => {
                write!(f, "migration of {} failed: {}", path.display(), error)
            }
        }
    }
}

pub type Result<T> = std::result::Result<T, PadzError>;
