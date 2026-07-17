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

/// If `title[start..]` begins with `term_lower` compared case-insensitively,
/// return the byte offset in `title` just past the match.
///
/// Every offset this returns is a real char boundary *in the original title*,
/// which is the point: `title.to_lowercase()` is not offset-compatible with
/// `title`. Lowercasing can change byte length (`İ` U+0130, 2 bytes, lowercases
/// to `i` + U+0307, 3 bytes), so an offset found in the lowercased copy can
/// land mid-char in the original and panic the slice. Matching walks the
/// original's chars and compares their lowercase expansion instead, so no
/// offset ever crosses between the two strings.
///
/// A char whose expansion only partially satisfies the remaining term (`İ`
/// against the term `i`) is not a match: reporting one would mean styling half
/// a char, which is not a slice we can take.
fn match_end(title: &str, start: usize, term_lower: &str) -> Option<usize> {
    let mut expected = term_lower.chars().peekable();
    for (offset, ch) in title[start..].char_indices() {
        if expected.peek().is_none() {
            return Some(start + offset);
        }
        for lowered in ch.to_lowercase() {
            match expected.next() {
                Some(want) if want == lowered => {}
                _ => return None,
            }
        }
    }
    // The term ran out exactly at the end of the title.
    expected.peek().is_none().then_some(title.len())
}

/// Render `title` plain, except for the substring matching `term_lower` (case-
/// insensitive), which gets the same yellow-background highlight as search hits.
fn style_title_with_match(title: &str, term_lower: &str) -> String {
    if term_lower.is_empty() {
        return title.to_string();
    }
    let mut out = String::with_capacity(title.len() + 16);
    // `plain_from` trails `cursor`: the run of unstyled text not yet flushed.
    let mut plain_from = 0usize;
    let mut cursor = 0usize;
    while cursor < title.len() {
        match match_end(title, cursor, term_lower) {
            Some(end) if end > cursor => {
                out.push_str(&title[plain_from..cursor]);
                out.push_str(
                    &console::style(&title[cursor..end])
                        .black()
                        .on_yellow()
                        .to_string(),
                );
                cursor = end;
                plain_from = end;
            }
            _ => {
                // Advance one whole char, never one byte.
                cursor += title[cursor..].chars().next().map_or(1, |ch| ch.len_utf8());
            }
        }
    }
    out.push_str(&title[plain_from..]);
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

    /// `match_end` is the offset logic the styling rides on. Asserting it
    /// directly keeps these cases independent of whether `console` decides to
    /// emit color in this environment.
    #[test]
    fn match_end_reports_offsets_into_the_original_title() {
        // Case-insensitive match at 0, ending past the ASCII term.
        assert_eq!(match_end("Meeting Monday", 0, "meeting"), Some(7));
        // No match at this offset.
        assert_eq!(match_end("Meeting Monday", 1, "meeting"), None);
        // Match starting after a multi-byte char: "Café " is 6 bytes.
        assert_eq!(match_end("Café Meeting", 6, "meeting"), Some(13));
        // The match itself is multi-byte: "Café" is 5 bytes.
        assert_eq!(match_end("Café Meeting", 0, "café"), Some(5));
        // Term longer than the remaining title.
        assert_eq!(match_end("Meet", 0, "meeting"), None);
        // Match runs to the exact end of the title.
        assert_eq!(match_end("The Meeting", 4, "meeting"), Some(11));
        // `İ` lowercases to 2 chars; the term `i` only covers the first, so
        // this is not a match rather than a half-char slice.
        assert_eq!(match_end("İstanbul", 0, "i"), None);
    }

    /// Every match in the title is highlighted, not just the first — and the
    /// text between and around them survives.
    #[test]
    fn every_occurrence_is_highlighted() {
        let styled = style_title_with_match("meeting about the meeting", "meeting");
        assert_eq!(
            console::strip_ansi_codes(&styled),
            "meeting about the meeting"
        );
    }

    /// Multi-byte titles must round-trip whether or not they match. Slicing on
    /// offsets taken from a lowercased copy used to panic here.
    #[test]
    fn unicode_titles_round_trip() {
        for (title, term) in [
            ("Café Meeting", "meeting"),  // match after multi-byte text
            ("Café Meeting", "café"),     // the match itself is multi-byte
            ("日本語のノート", "ノート"), // no ASCII at all
            ("Ünïcödé", "z"),             // no match, all multi-byte
            ("naïve café", "CAFÉ"),       // uppercase multi-byte term
        ] {
            let styled = style_title_with_match(title, &term.to_lowercase());
            assert_eq!(
                console::strip_ansi_codes(&styled),
                title,
                "title {title:?} with term {term:?} must survive intact"
            );
        }
    }

    /// The regression the offset bug hid behind: `İ` (U+0130) is 2 bytes and
    /// lowercases to 3 (`i` + U+0307), so offsets from the lowercased copy fall
    /// mid-char in the original. Must not panic, and must not corrupt the title.
    #[test]
    fn length_changing_lowercase_does_not_panic() {
        for title in ["İstanbul", "İİİ", "aİb", "İ"] {
            for term in ["i", "istanbul", "a", "İ"] {
                let styled = style_title_with_match(title, &term.to_lowercase());
                assert_eq!(
                    console::strip_ansi_codes(&styled),
                    title,
                    "title {title:?} with term {term:?} must survive intact"
                );
            }
        }
    }

    /// A term longer than the title, and a match running to the exact end,
    /// exercise the two boundary paths in `match_end`.
    #[test]
    fn match_at_title_boundaries() {
        // Term longer than the remaining title → no match, title intact.
        assert_eq!(style_title_with_match("Meet", "meeting"), "Meet");
        // Match ends exactly at the end of the title.
        let styled = style_title_with_match("The Meeting", "meeting");
        assert_eq!(console::strip_ansi_codes(&styled), "The Meeting");
        // Whole title is the match.
        let styled = style_title_with_match("Meeting", "meeting");
        assert_eq!(console::strip_ansi_codes(&styled), "Meeting");
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
