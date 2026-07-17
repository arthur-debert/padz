//! Terminal presentation of errors.
//!
//! `padzapp` returns errors as data — notably
//! [`PadzError::AmbiguousTitle`], which carries the matched pads as
//! [`AmbiguityCandidate`] values rather than a pre-formatted string. This
//! module is where that data becomes the styled text a person reads, using the
//! same accents the list/search renderer uses so an ambiguity error looks like
//! the listing it refers to.
//!
//! Every other error renders through its `Display`. `console::style` collapses
//! to plain text on its own when stderr is not a TTY or the terminal can't take
//! color, so no `IsTerminal` checks are needed here.

use padzapp::error::{AmbiguityCandidate, PadzError};

/// Converts a library error into the `anyhow::Error` handlers return, styling
/// it on the way.
///
/// **This is where an error's presentation is decided**, and it has to be:
/// standout carries a dispatch failure as `RunResult::Error(String)`, so a
/// `PadzError` is flattened to text here, at the handler boundary, long before
/// `main` sees it. Anything that wants to look at the error's *structure* — as
/// [`render`] does for [`PadzError::AmbiguousTitle`] — must do it now or not
/// at all.
pub fn to_anyhow(err: PadzError) -> anyhow::Error {
    anyhow::anyhow!("{}", render(&err))
}

/// Renders an error for stderr, styling the ones we have a richer
/// presentation for and falling back to `Display` for the rest.
pub fn render(err: &PadzError) -> String {
    match err {
        PadzError::AmbiguousTitle {
            term,
            total,
            candidates,
        } => render_ambiguity(term, *total, candidates),
        other => other.to_string(),
    }
}

/// Styles an ambiguous-title error.
///
/// With candidates, enumerates them as an indented list; without (the match
/// count was too high to be worth listing), reports the count alone. Mirrors
/// `padzapp::error`'s plain rendering, with color added.
fn render_ambiguity(term: &str, total: usize, candidates: &[AmbiguityCandidate]) -> String {
    if candidates.is_empty() {
        return format!(
            "Term {} matches {} pads. Please be more specific.",
            style_match(term),
            total
        );
    }
    let term_lower = term.to_lowercase();
    let listing = candidates
        .iter()
        .map(|c| {
            format!(
                "    {}. {}",
                style_index(&c.index),
                style_title_with_match(&c.title, &term_lower)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "Term {} matches multiple pads. Use one, or be more specific:\n{}",
        style_match(term),
        listing
    )
}

/// Format a display index in the same accent color the list/search renderer
/// uses for `list-index` (gold/yellow).
fn style_index(s: &str) -> String {
    console::style(s).yellow().to_string()
}

/// Format the search term with the same yellow-background highlight the list/
/// search renderer uses for `match` hits (yellow bg, black fg). Wraps the
/// styled term in quotes so the message reads `Term "foo" matches ...`.
fn style_match(term: &str) -> String {
    format!("\"{}\"", console::style(term).black().on_yellow())
}

/// Render `title` plain, except for the substring matching `term_lower` (case-
/// insensitive), which gets the same yellow-background highlight as search hits.
fn style_title_with_match(title: &str, term_lower: &str) -> String {
    if term_lower.is_empty() {
        return title.to_string();
    }
    let title_lower = title.to_lowercase();
    let mut out = String::with_capacity(title.len() + 16);
    let mut cursor = 0usize;
    while let Some(rel) = title_lower[cursor..].find(term_lower) {
        let start = cursor + rel;
        let end = start + term_lower.len();
        out.push_str(&title[cursor..start]);
        out.push_str(
            &console::style(&title[start..end])
                .black()
                .on_yellow()
                .to_string(),
        );
        cursor = end;
    }
    out.push_str(&title[cursor..]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(index: &str, title: &str) -> AmbiguityCandidate {
        AmbiguityCandidate {
            index: index.to_string(),
            title: title.to_string(),
        }
    }

    /// The listing carries every candidate's index and title. Asserted on
    /// ANSI-stripped text so the test is stable regardless of whether
    /// `console` decides to emit color in this environment.
    #[test]
    fn ambiguity_listing_names_every_candidate() {
        let rendered = render_ambiguity(
            "meeting",
            2,
            &[
                candidate("1", "Meeting Monday"),
                candidate("p2", "Meeting Tuesday"),
            ],
        );
        let plain = console::strip_ansi_codes(&rendered).to_string();
        assert!(plain.contains("multiple pads"), "got: {plain}");
        assert!(plain.contains("    1. Meeting Monday"), "got: {plain}");
        assert!(plain.contains("    p2. Meeting Tuesday"), "got: {plain}");
    }

    /// No candidates → the count-only message, and no invented listing.
    #[test]
    fn ambiguity_without_candidates_reports_count_only() {
        let rendered = render_ambiguity("meeting", 6, &[]);
        let plain = console::strip_ansi_codes(&rendered).to_string();
        assert!(plain.contains("6 pads"), "got: {plain}");
        assert!(plain.contains("Please be more specific"), "got: {plain}");
        assert!(
            !plain.contains('\n'),
            "count-only must be one line: {plain}"
        );
    }

    /// A title match is highlighted case-insensitively, and the title's own
    /// casing survives — the user sees their pad's title, not a lowercased one.
    #[test]
    fn title_match_highlight_preserves_original_casing() {
        let styled = style_title_with_match("Meeting Monday", "meeting");
        assert_eq!(console::strip_ansi_codes(&styled), "Meeting Monday");
    }

    /// An empty term must not loop forever looking for an empty needle.
    #[test]
    fn empty_term_returns_title_unstyled() {
        assert_eq!(style_title_with_match("Meeting", ""), "Meeting");
    }

    /// `render` styles the variant it knows and passes everything else
    /// through to `Display`.
    #[test]
    fn render_falls_back_to_display_for_other_errors() {
        let err = PadzError::Api("boom".to_string());
        assert_eq!(render(&err), "Api Error: boom");
    }

    #[test]
    fn render_styles_ambiguous_title() {
        let err = PadzError::AmbiguousTitle {
            term: "meeting".to_string(),
            total: 1,
            candidates: vec![candidate("1", "Meeting Monday")],
        };
        let plain = console::strip_ansi_codes(&render(&err)).to_string();
        assert!(plain.contains("    1. Meeting Monday"), "got: {plain}");
    }
}
