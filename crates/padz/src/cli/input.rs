//! Request-scoped input resolution for `create`/`edit`, and naked invocation.
//!
//! # Why this module exists
//!
//! Deciding *where a pad's text comes from* is a shell concern, not a domain
//! one: it depends on flags, on whether stdin is a pipe, and on whether a
//! terminal is attached. That decision used to live inside the `create` and
//! `edit` handlers, which read stdin and probed `is_terminal()` directly. It
//! now happens here, before dispatch, and the handlers receive one resolved,
//! typed [`RequestContent`] telling them what to do.
//!
//! The precedence itself is expressed as a Standout [`InputChain`]: each source
//! declares when it is available, the chain tries them in order, and the
//! `.default(...)` arm is the editor. Reading the chain is reading the policy.
//!
//! # The precedence (unchanged from before this module existed)
//!
//! For `create`:
//!
//! 1. **Direct** — the title args, used verbatim with the editor skipped. Only
//!    when `--no-editor` is set, or todos mode was given title args, and never
//!    when `--editor` forces the editor. **Stdin is not read at all on this
//!    path**, even when something is piped.
//! 2. **Piped** — non-empty piped stdin.
//! 3. **PipedEmpty** — stdin was piped but empty; the user aborted.
//! 4. **Editor** — nothing else offered content, so open the editor.
//!
//! For `edit` the same shape holds; only the Direct rule differs (todos mode
//! with trailing content words after the index selectors).
//!
//! # What is deliberately *not* a Standout primitive here
//!
//! - **`StdinSource`**: the framework source returns `None` for empty piped
//!   input, which in a chain means "fall through to the next source". Padz
//!   must *abort* on an empty pipe instead — falling through would silently
//!   open an editor where padz used to abort. So [`PipedSource`] reuses the
//!   framework's [`StdinReader`] seam (and therefore its test overrides) but
//!   owns the empty-input semantics, surfacing them as
//!   [`RequestContent::PipedEmpty`] rather than swallowing them.
//! - **`EditorSource`**: the framework source edits a scratch buffer and hands
//!   back a string. Padz creates the pad first and opens the editor on the real
//!   file in `.padz/`, then refreshes from disk — and on failure deletes the
//!   half-created pad. Those are pad-lifecycle concerns the handler must own,
//!   so the chain resolves to [`RequestContent::Editor`] and stops there; it
//!   does not try to launch anything.
//! - **`ClipboardSource`**: padz does not, and did not, read the clipboard as a
//!   create/edit input source. See [`naked_command`]'s sibling note in
//!   `cli::mod` docs — the clipboard is written to after a pad is saved, never
//!   read from to prefill one.
//! - **`App::default_command`**: it names one static command, but padz's naked
//!   invocation picks between two depending on whether stdin is piped. See
//!   [`naked_command`].

use clap::ArgMatches;
use padzapp::config::PadzMode;
use standout::input::env::{DefaultStdin, StdinReader};
use standout::input::{InputChain, InputCollector, InputError};
use std::sync::Arc;

// `split_indexes_and_content` is the handlers' own arg-splitting rule; the edit
// source must apply the same one to know whether trailing words are content.
use super::handlers::split_indexes_and_content;

/// The name the `create` content input is registered and looked up under.
pub const CREATE_CONTENT: &str = "create_content";

/// The name the `edit` content input is registered and looked up under.
pub const EDIT_CONTENT: &str = "edit_content";

/// Where a create/edit request's text comes from, resolved before dispatch.
///
/// This is the whole of what the handlers learn about the shell: they match on
/// it instead of probing stdin themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestContent {
    /// Text taken verbatim from the command line; the editor is skipped.
    ///
    /// Carries the args already joined and with literal `\n` expanded, exactly
    /// as the quick-create/quick-edit paths always did.
    Direct(String),
    /// Non-empty text read from piped stdin (trimmed).
    Piped(String),
    /// Stdin was piped but held nothing but whitespace — the user aborted.
    ///
    /// Distinct from [`RequestContent::Editor`] on purpose: an empty pipe is an
    /// abort, not an invitation to open an editor on a non-interactive stdin.
    PipedEmpty,
    /// No non-interactive source offered text; open the editor on the pad file.
    Editor,
}

// =============================================================================
// Sources
// =============================================================================

/// The `create` quick-path source: title args used verbatim, editor skipped.
///
/// Availability mirrors the original `skip_editor` rule exactly:
/// `!--editor && (--no-editor || (todos mode && title args present))`.
struct CreateDirectSource {
    mode: PadzMode,
}

impl InputCollector<RequestContent> for CreateDirectSource {
    // "argument" (not "args"): `InputChain::try_source` maps the *name string*
    // to an `InputSourceKind`, and anything it does not recognize silently
    // becomes `Default`. This spelling is what reports the value as `Arg`.
    fn name(&self) -> &'static str {
        "argument"
    }

    fn is_available(&self, matches: &ArgMatches) -> bool {
        let editor = flag(matches, "editor");
        let no_editor = flag(matches, "no_editor");
        let has_title = !title_args(matches).is_empty();
        !editor && (no_editor || (self.mode == PadzMode::Todos && has_title))
    }

    fn collect(&self, matches: &ArgMatches) -> Result<Option<RequestContent>, InputError> {
        let raw = title_args(matches).join(" ");
        Ok(Some(RequestContent::Direct(expand_newlines(&raw))))
    }
}

/// The `edit` quick-path source: trailing content words in todos mode.
///
/// `edit`'s positional args mix index selectors and content words, so
/// availability depends on the same split the handler uses for the selectors.
struct EditDirectSource {
    mode: PadzMode,
}

impl EditDirectSource {
    /// The trailing content words, when todos mode makes them a quick-edit.
    fn inline_content(&self, matches: &ArgMatches) -> Option<String> {
        if self.mode != PadzMode::Todos {
            return None;
        }
        let (_, content_words) = split_indexes_and_content(&index_args(matches));
        if content_words.is_empty() {
            return None;
        }
        Some(expand_newlines(&content_words.join(" ")))
    }
}

impl InputCollector<RequestContent> for EditDirectSource {
    // See `CreateDirectSource::name` — the string is the kind mapping.
    fn name(&self) -> &'static str {
        "argument"
    }

    fn is_available(&self, matches: &ArgMatches) -> bool {
        self.inline_content(matches).is_some()
    }

    fn collect(&self, matches: &ArgMatches) -> Result<Option<RequestContent>, InputError> {
        Ok(self.inline_content(matches).map(RequestContent::Direct))
    }
}

/// Piped stdin, with padz's abort-on-empty semantics.
///
/// Reuses the framework's [`StdinReader`] abstraction — so
/// `standout::input::env::set_default_stdin_reader` overrides work here just as
/// they do for the framework's own source — but deliberately does **not** reuse
/// `StdinSource`, whose empty-input `None` would let the chain fall through to
/// the editor. Here an empty pipe resolves to [`RequestContent::PipedEmpty`],
/// which the handlers turn into the same abort they always produced.
struct PipedSource {
    reader: Arc<dyn StdinReader>,
}

impl PipedSource {
    /// Reads the process's real stdin, honoring any installed test override.
    fn from_process() -> Self {
        Self {
            reader: Arc::new(DefaultStdin),
        }
    }

    /// Reads from an injected reader. Used by tests to drive both the piped and
    /// terminal cases without a pty and without touching the real stdin.
    #[cfg(test)]
    fn with_reader(reader: impl StdinReader + 'static) -> Self {
        Self {
            reader: Arc::new(reader),
        }
    }
}

impl InputCollector<RequestContent> for PipedSource {
    fn name(&self) -> &'static str {
        "stdin"
    }

    fn is_available(&self, _matches: &ArgMatches) -> bool {
        // A terminal stdin is not a source of content; it means "open the editor".
        !self.reader.is_terminal()
    }

    fn collect(&self, _matches: &ArgMatches) -> Result<Option<RequestContent>, InputError> {
        if self.reader.is_terminal() {
            return Ok(None);
        }
        let raw = self
            .reader
            .read_to_string()
            .map_err(InputError::StdinFailed)?;
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            // Never `None`: falling through to the editor would silently
            // replace padz's abort-on-empty-pipe behavior.
            return Ok(Some(RequestContent::PipedEmpty));
        }
        Ok(Some(RequestContent::Piped(trimmed.to_string())))
    }
}

// =============================================================================
// Chains
// =============================================================================

/// The `create` content chain: direct args, then piped stdin, then the editor.
pub(super) fn create_chain(mode: PadzMode) -> InputChain<RequestContent> {
    InputChain::new()
        .try_source(CreateDirectSource { mode })
        .try_source(PipedSource::from_process())
        .default(RequestContent::Editor)
}

/// The `edit` content chain: inline todos content, then piped stdin, then the editor.
pub(super) fn edit_chain(mode: PadzMode) -> InputChain<RequestContent> {
    InputChain::new()
        .try_source(EditDirectSource { mode })
        .try_source(PipedSource::from_process())
        .default(RequestContent::Editor)
}

// =============================================================================
// Naked invocation
// =============================================================================

/// Which command a naked `padz` stands for: `create` when stdin is piped,
/// `list` otherwise.
///
/// # Why this is not `App::default_command`
///
/// Standout 7.6 has a declarative default command, but it names exactly one
/// static command (`insert_default_command` splices that literal into argv).
/// Padz's rule is *conditional* — the whole point is that `cat notes | padz`
/// captures while a bare `padz` reads — and 7.6 offers no predicate hook for
/// that. Standout's default also only fires inside `App::run`, and padz drives
/// `parse_from` + `dispatch` so it can build app state and resolve the output
/// mode first, so the setting would not apply on that path regardless.
///
/// So this stays application-owned, but it is a pure function of a
/// [`StdinReader`] rather than an inline `is_terminal()` probe, which is what
/// makes both arms testable without a pty.
pub fn naked_command(reader: &dyn StdinReader) -> &'static str {
    if reader.is_terminal() {
        "list"
    } else {
        "create"
    }
}

/// [`naked_command`] against the process's real stdin (honoring test overrides).
pub fn naked_command_from_process() -> &'static str {
    naked_command(&DefaultStdin)
}

// =============================================================================
// ArgMatches helpers
// =============================================================================

fn flag(matches: &ArgMatches, name: &str) -> bool {
    matches.try_get_one::<bool>(name).ok().flatten() == Some(&true)
}

fn title_args(matches: &ArgMatches) -> Vec<String> {
    many(matches, "title")
}

fn index_args(matches: &ArgMatches) -> Vec<String> {
    many(matches, "indexes")
}

fn many(matches: &ArgMatches, name: &str) -> Vec<String> {
    matches
        .try_get_many::<String>(name)
        .ok()
        .flatten()
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default()
}

/// Turns the literal two-character sequence `\n` into a real newline.
///
/// Shells make it awkward to put a real newline in an argument, so padz has
/// always accepted `padz create 'Title\nBody'` on the quick paths.
fn expand_newlines(raw: &str) -> String {
    raw.replace("\\n", "\n")
}

/// Resolves `chain`, returning the value and the source kind that produced it,
/// so tests can assert on provenance as well as the value.
#[cfg(test)]
fn resolve_for_test(
    chain: InputChain<RequestContent>,
    matches: &ArgMatches,
) -> (RequestContent, standout::input::InputSourceKind) {
    let r = chain.resolve_with_source(matches).expect("chain resolves");
    (r.value, r.source)
}

#[cfg(test)]
mod tests {
    use super::*;
    use standout::input::env::MockStdin;
    use standout::input::InputSourceKind;

    /// Builds `create`'s clap matches the way the real command parses them.
    fn create_matches(args: &[&str]) -> ArgMatches {
        clap::Command::new("create")
            .arg(
                clap::Arg::new("editor")
                    .long("editor")
                    .short('e')
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("no_editor")
                    .long("no-editor")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("title")
                    .num_args(0..)
                    .trailing_var_arg(true)
                    .action(clap::ArgAction::Append),
            )
            .try_get_matches_from(std::iter::once("create").chain(args.iter().copied()))
            .expect("test args parse")
    }

    fn edit_matches(args: &[&str]) -> ArgMatches {
        clap::Command::new("edit")
            .arg(
                clap::Arg::new("indexes")
                    .num_args(1..)
                    .action(clap::ArgAction::Append),
            )
            .try_get_matches_from(std::iter::once("edit").chain(args.iter().copied()))
            .expect("test args parse")
    }

    /// A create chain with an injected stdin, so no test needs a pty.
    fn create_chain_with(mode: PadzMode, stdin: MockStdin) -> InputChain<RequestContent> {
        InputChain::new()
            .try_source(CreateDirectSource { mode })
            .try_source(PipedSource::with_reader(stdin))
            .default(RequestContent::Editor)
    }

    fn edit_chain_with(mode: PadzMode, stdin: MockStdin) -> InputChain<RequestContent> {
        InputChain::new()
            .try_source(EditDirectSource { mode })
            .try_source(PipedSource::with_reader(stdin))
            .default(RequestContent::Editor)
    }

    // --- create: the direct/quick path ---

    /// `--no-editor` takes the args verbatim — and does not read stdin, even
    /// though something is piped. This is the precedence, pinned.
    #[test]
    fn no_editor_wins_over_piped_stdin() {
        let (value, source) = resolve_for_test(
            create_chain_with(PadzMode::Notes, MockStdin::piped("IGNORED")),
            &create_matches(&["--no-editor", "ArgTitle"]),
        );
        assert_eq!(value, RequestContent::Direct("ArgTitle".into()));
        assert_eq!(source, InputSourceKind::Arg);
    }

    /// Todos mode with title args skips the editor; notes mode does not.
    #[test]
    fn todos_mode_with_title_takes_the_direct_path() {
        let (value, _) = resolve_for_test(
            create_chain_with(PadzMode::Todos, MockStdin::terminal()),
            &create_matches(&["Buy", "milk"]),
        );
        assert_eq!(value, RequestContent::Direct("Buy milk".into()));
    }

    #[test]
    fn notes_mode_with_title_still_opens_the_editor() {
        let (value, _) = resolve_for_test(
            create_chain_with(PadzMode::Notes, MockStdin::terminal()),
            &create_matches(&["A", "note"]),
        );
        assert_eq!(value, RequestContent::Editor);
    }

    /// Todos mode without title args has nothing to quick-create from.
    #[test]
    fn todos_mode_without_title_opens_the_editor() {
        let (value, _) = resolve_for_test(
            create_chain_with(PadzMode::Todos, MockStdin::terminal()),
            &create_matches(&[]),
        );
        assert_eq!(value, RequestContent::Editor);
    }

    /// `--editor` forces the editor even in todos mode with title args.
    #[test]
    fn editor_flag_defeats_the_direct_path() {
        let (value, _) = resolve_for_test(
            create_chain_with(PadzMode::Todos, MockStdin::terminal()),
            &create_matches(&["--editor", "Buy", "milk"]),
        );
        assert_eq!(value, RequestContent::Editor);
    }

    /// Literal `\n` in an arg becomes a real newline on the direct path.
    #[test]
    fn direct_path_expands_literal_newlines() {
        let (value, _) = resolve_for_test(
            create_chain_with(PadzMode::Notes, MockStdin::terminal()),
            &create_matches(&["--no-editor", r"Title\nBody line"]),
        );
        assert_eq!(value, RequestContent::Direct("Title\nBody line".into()));
    }

    /// `--no-editor` with no title at all still takes the direct path, with
    /// empty text — which is how padz has always produced an empty pad here.
    #[test]
    fn no_editor_without_title_is_direct_and_empty() {
        let (value, _) = resolve_for_test(
            create_chain_with(PadzMode::Notes, MockStdin::piped("IGNORED")),
            &create_matches(&["--no-editor"]),
        );
        assert_eq!(value, RequestContent::Direct(String::new()));
    }

    // --- create: stdin ---

    #[test]
    fn piped_stdin_is_used_when_no_direct_source() {
        let (value, source) = resolve_for_test(
            create_chain_with(PadzMode::Notes, MockStdin::piped("  Piped body  ")),
            &create_matches(&[]),
        );
        assert_eq!(value, RequestContent::Piped("Piped body".into()));
        assert_eq!(source, InputSourceKind::Stdin);
    }

    /// The regression this module's `PipedSource` exists for: an empty pipe is
    /// an abort, never a fall-through to the editor.
    #[test]
    fn empty_pipe_aborts_rather_than_opening_the_editor() {
        let (value, _) = resolve_for_test(
            create_chain_with(PadzMode::Notes, MockStdin::piped_empty()),
            &create_matches(&[]),
        );
        assert_eq!(value, RequestContent::PipedEmpty);
    }

    /// Whitespace-only is empty too — the old code trimmed before testing.
    #[test]
    fn whitespace_only_pipe_aborts() {
        let (value, _) = resolve_for_test(
            create_chain_with(PadzMode::Notes, MockStdin::piped("   \n  \n")),
            &create_matches(&[]),
        );
        assert_eq!(value, RequestContent::PipedEmpty);
    }

    /// A terminal stdin means "no piped content": open the editor.
    #[test]
    fn terminal_stdin_falls_through_to_the_editor() {
        let (value, source) = resolve_for_test(
            create_chain_with(PadzMode::Notes, MockStdin::terminal()),
            &create_matches(&[]),
        );
        assert_eq!(value, RequestContent::Editor);
        assert_eq!(source, InputSourceKind::Default);
    }

    // --- edit ---

    #[test]
    fn todos_edit_with_trailing_words_is_direct() {
        let (value, _) = resolve_for_test(
            edit_chain_with(PadzMode::Todos, MockStdin::terminal()),
            &edit_matches(&["1", "Edited", "text"]),
        );
        assert_eq!(value, RequestContent::Direct("Edited text".into()));
    }

    #[test]
    fn todos_edit_expands_literal_newlines() {
        let (value, _) = resolve_for_test(
            edit_chain_with(PadzMode::Todos, MockStdin::terminal()),
            &edit_matches(&["1", r"Edited\nBody"]),
        );
        assert_eq!(value, RequestContent::Direct("Edited\nBody".into()));
    }

    /// Notes mode never quick-edits, even with trailing words.
    #[test]
    fn notes_edit_with_trailing_words_opens_the_editor() {
        let (value, _) = resolve_for_test(
            edit_chain_with(PadzMode::Notes, MockStdin::terminal()),
            &edit_matches(&["1", "Edited", "text"]),
        );
        assert_eq!(value, RequestContent::Editor);
    }

    /// Index-only args are not content, so there is nothing to quick-edit from.
    #[test]
    fn todos_edit_with_only_indexes_opens_the_editor() {
        let (value, _) = resolve_for_test(
            edit_chain_with(PadzMode::Todos, MockStdin::terminal()),
            &edit_matches(&["1", "2"]),
        );
        assert_eq!(value, RequestContent::Editor);
    }

    #[test]
    fn edit_reads_piped_stdin() {
        let (value, _) = resolve_for_test(
            edit_chain_with(PadzMode::Notes, MockStdin::piped("New body")),
            &edit_matches(&["1"]),
        );
        assert_eq!(value, RequestContent::Piped("New body".into()));
    }

    #[test]
    fn edit_empty_pipe_aborts() {
        let (value, _) = resolve_for_test(
            edit_chain_with(PadzMode::Notes, MockStdin::piped_empty()),
            &edit_matches(&["1"]),
        );
        assert_eq!(value, RequestContent::PipedEmpty);
    }

    /// Todos inline content beats a pipe, mirroring create's direct path.
    #[test]
    fn todos_edit_inline_content_wins_over_stdin() {
        let (value, _) = resolve_for_test(
            edit_chain_with(PadzMode::Todos, MockStdin::piped("PIPED")),
            &edit_matches(&["1", "Inline"]),
        );
        assert_eq!(value, RequestContent::Direct("Inline".into()));
    }

    // --- naked invocation ---

    #[test]
    fn naked_padz_lists_on_a_terminal() {
        assert_eq!(naked_command(&MockStdin::terminal()), "list");
    }

    #[test]
    fn naked_padz_creates_when_piped() {
        assert_eq!(naked_command(&MockStdin::piped("captured")), "create");
    }

    /// An empty pipe is still a pipe: naked padz routes to `create`, which then
    /// aborts on the empty content rather than listing.
    #[test]
    fn naked_padz_creates_even_when_the_pipe_is_empty() {
        assert_eq!(naked_command(&MockStdin::piped_empty()), "create");
    }
}
