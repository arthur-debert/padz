//! Command handlers for padz CLI.
//!
//! All handlers use the `#[handler]` proc macro from `standout_macros` which auto-extracts
//! CLI arguments from clap's ArgMatches. The handler signature uses annotations:
//! - `#[ctx]` - CommandContext reference
//! - `#[arg]` / `#[arg(name = "x")]` - Positional/named arguments
//! - `#[flag]` / `#[flag(name = "x")]` - Boolean flags
//!
//! Each handler with `#[handler]` generates a `{name}__handler` wrapper function
//! (non-snake-case by design) that dispatch calls via `#[dispatch(pure)]` in setup.rs.
//!
//! State is accessed via standout's app_state mechanism, injected at app build time.

// Allow non_snake_case for macro-generated __handler wrapper functions
#![allow(non_snake_case)]

use crate::cli::clipboard::{format_for_clipboard, ClipboardWriter, SystemClipboardWriter};
use crate::cli::errors::to_anyhow;
use crate::cli::input::{RequestContent, CREATE_CONTENT, EDIT_CONTENT};
use padzapp::api::{PadFilter, PadStatusFilter, PadzApi, TodoStatus};
use padzapp::commands::{CmdOutcome, CmdResult, NestingMode, UpdateKind};
use padzapp::config::PadzMode;
use padzapp::error::PadzError;
use padzapp::model::{extract_title_and_body, Scope};
use padzapp::store::fs::FileStore;
use standout::cli::{Artifact, CommandContext, CommandContextInput, Output};
use standout_macros::handler;
use std::cell::RefCell;
use std::rc::Rc;

use super::result::{
    ListRequest, Listing, Modification, ModificationAction, ModificationRequest, PadContent,
    PadContentResult,
};
use super::views::{CopyView, PathView, UuidView};
use padzapp::commands::doctor::DoctorOutcome;
use padzapp::commands::init::InitializationOutcome;
use padzapp::commands::purge::PurgeOutcome;
use padzapp::commands::tagging::TaggingResult;
use padzapp::commands::tags::{TagCatalogOutcome, TagRegistryOutcome};

// =============================================================================
// App State Types (for standout's type-based app_state lookup)
// =============================================================================

/// Wrapper for import extensions list (needed for type-based lookup)
#[derive(Clone)]
pub struct ImportExtensions(pub Vec<String>);

/// Shared application state injected via app_state.
///
/// Contains the API instance wrapped in `RefCell` for interior mutability and
/// the CLI-owned clipboard destination used for best-effort final writes.
///
/// State is deliberately free of `OutputMode`: handlers return one typed result
/// regardless of `--output`, and the output mode is resolved at the app-execution
/// boundary (see [`super::commands`]).
pub struct AppState {
    api: RefCell<PadzApi<FileStore>>,
    clipboard: Rc<dyn ClipboardWriter>,
    pub scope: Scope,
    pub import_extensions: ImportExtensions,
    pub mode: PadzMode,
    /// The local `.padz/` directory (pre-link-resolution), used by link/unlink commands.
    pub local_padz_dir: std::path::PathBuf,
}

impl AppState {
    pub fn new(
        api: PadzApi<FileStore>,
        scope: Scope,
        import_extensions: Vec<String>,
        mode: PadzMode,
        local_padz_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            api: RefCell::new(api),
            clipboard: Rc::new(SystemClipboardWriter),
            scope,
            import_extensions: ImportExtensions(import_extensions),
            mode,
            local_padz_dir,
        }
    }

    /// Replace the platform clipboard destination.
    ///
    /// Production assembly keeps the default system writer. In-process CLI tests
    /// supply a recording writer so they can prove one ordered write from semantic
    /// pad data without invoking an OS clipboard command.
    pub fn with_clipboard_writer(mut self, clipboard: Rc<dyn ClipboardWriter>) -> Self {
        self.clipboard = clipboard;
        self
    }

    /// Best-effort clipboard write, preserving Padz's established failure semantics.
    fn copy_to_clipboard(&self, text: &str) {
        let _ = self.clipboard.write(text);
    }

    /// Whether todo status icons are part of this invocation's request.
    ///
    /// Todos mode asks for them implicitly; `force` covers commands that change
    /// status (`complete`, `reopen`) and the explicit `--show-status` flag.
    /// Mode-independent: it says what to include, not how to draw it.
    pub fn wants_status(&self, force: bool) -> bool {
        force || self.mode == PadzMode::Todos
    }

    /// Access the API with mutable borrow
    pub fn with_api<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut PadzApi<FileStore>) -> R,
    {
        f(&mut self.api.borrow_mut())
    }
}

/// Helper to get AppState from CommandContext
fn get_state(ctx: &CommandContext) -> &AppState {
    ctx.app_state
        .get::<AppState>()
        .expect("AppState not initialized in app_state")
}

// =============================================================================
// Scoped API - eliminates handler boilerplate
// =============================================================================

/// Scoped API accessor that binds scope, converts errors, and maps `CmdResult` into
/// the CLI's typed result contract.
///
/// This wrapper eliminates the repetitive boilerplate pattern:
/// ```ignore
/// let state = get_state(ctx);
/// let result = state.with_api(|api| {
///     api.method(state.scope, &args).map_err(to_anyhow)
/// })?;
/// Ok(Output::Render(Modification { ... }))
/// ```
///
/// With ScopedApi, handlers become one-liners:
/// ```ignore
/// api(ctx).pin_pads(&indexes)
/// ```
///
/// Nothing here renders or prints: it returns the same typed value whatever
/// `--output` asked for.
pub struct ScopedApi<'a> {
    state: &'a AppState,
}

impl<'a> ScopedApi<'a> {
    /// Execute an API call with scope bound and error conversion
    fn call<T, F>(&self, f: F) -> Result<T, anyhow::Error>
    where
        F: FnOnce(&mut PadzApi<FileStore>, Scope) -> Result<T, PadzError>,
    {
        self.state
            .with_api(|api| f(api, self.state.scope).map_err(to_anyhow))
    }

    /// Map a CmdResult (modification) into the thin modification view.
    /// When `force_show_status` is true, status icons are requested regardless of mode.
    fn modification(
        &self,
        action: ModificationAction,
        result: CmdResult,
        force_show_status: bool,
    ) -> Result<Output<Modification>, anyhow::Error> {
        Ok(Output::Render(self.modification_result(
            action,
            result,
            force_show_status,
        )))
    }

    /// Build the thin modification view without wrapping it in `Output`.
    ///
    /// The core's `notices`/`outcomes` (`CmdNotice`/`CmdOutcome`) ride through
    /// verbatim — no projection — so the template and structured output read the
    /// same core shape. Used by the handlers that assemble a `CmdResult`
    /// themselves (create, edit).
    fn modification_result(
        &self,
        action: ModificationAction,
        result: CmdResult,
        force_show_status: bool,
    ) -> Modification {
        Modification {
            action,
            pads: result.affected_pads,
            notices: result.notices,
            outcomes: result.outcomes,
            request: ModificationRequest {
                status: self.state.wants_status(force_show_status),
            },
        }
    }

    // --- Modification operations ---

    pub fn pin_pads(&self, indexes: &[String]) -> Result<Output<Modification>, anyhow::Error> {
        let result = self.call(|api, scope| api.pin_pads(scope, indexes))?;
        self.modification(ModificationAction::Pin, result, false)
    }

    pub fn unpin_pads(&self, indexes: &[String]) -> Result<Output<Modification>, anyhow::Error> {
        let result = self.call(|api, scope| api.unpin_pads(scope, indexes))?;
        self.modification(ModificationAction::Unpin, result, false)
    }

    pub fn delete_pads(&self, indexes: &[String]) -> Result<Output<Modification>, anyhow::Error> {
        let result = self.call(|api, scope| api.delete_pads(scope, indexes))?;
        self.modification(ModificationAction::Delete, result, false)
    }

    pub fn delete_completed_pads(&self) -> Result<Output<Modification>, anyhow::Error> {
        let result = self.call(|api, scope| api.delete_completed_pads(scope))?;
        self.modification(ModificationAction::Delete, result, false)
    }

    pub fn restore_pads(&self, indexes: &[String]) -> Result<Output<Modification>, anyhow::Error> {
        let result = self.call(|api, scope| api.restore_pads(scope, indexes))?;
        self.modification(ModificationAction::Restore, result, false)
    }

    pub fn archive_pads(&self, indexes: &[String]) -> Result<Output<Modification>, anyhow::Error> {
        let result = self.call(|api, scope| api.archive_pads(scope, indexes))?;
        self.modification(ModificationAction::Archive, result, false)
    }

    pub fn unarchive_pads(
        &self,
        indexes: &[String],
    ) -> Result<Output<Modification>, anyhow::Error> {
        let result = self.call(|api, scope| api.unarchive_pads(scope, indexes))?;
        self.modification(ModificationAction::Unarchive, result, false)
    }

    pub fn complete_pads(&self, indexes: &[String]) -> Result<Output<Modification>, anyhow::Error> {
        let result = self.call(|api, scope| api.complete_pads(scope, indexes))?;
        self.modification(ModificationAction::Complete, result, true)
    }

    pub fn reopen_pads(&self, indexes: &[String]) -> Result<Output<Modification>, anyhow::Error> {
        let result = self.call(|api, scope| api.reopen_pads(scope, indexes))?;
        self.modification(ModificationAction::Reopen, result, true)
    }

    pub fn move_pads(
        &self,
        indexes: &[String],
        to_root: bool,
    ) -> Result<Output<Modification>, anyhow::Error> {
        let result = if to_root {
            self.call(|api, scope| api.move_pads(scope, indexes, None))?
        } else {
            if indexes.len() < 2 {
                return Err(anyhow::anyhow!(
                    "Move requires at least 2 arguments (source and destination) or --root flag"
                ));
            }
            let (sources, dest) = indexes.split_at(indexes.len() - 1);
            self.call(|api, scope| api.move_pads(scope, sources, Some(dest[0].as_str())))?
        };
        self.modification(ModificationAction::Move, result, false)
    }

    // --- Maintenance outcomes ---

    pub fn purge_pads(
        &self,
        indexes: &[String],
        yes: bool,
        recursive: bool,
    ) -> Result<Output<PurgeOutcome>, anyhow::Error> {
        let include_done = self.state.mode == PadzMode::Todos;
        let outcome =
            self.call(|api, scope| api.purge_pads(scope, indexes, recursive, yes, include_done))?;
        Ok(Output::Render(outcome))
    }

    pub fn doctor(&self) -> Result<Output<DoctorOutcome>, anyhow::Error> {
        let outcome = self.call(|api, scope| api.doctor(scope))?;
        Ok(Output::Render(outcome))
    }

    pub fn init(&self) -> Result<Output<InitializationOutcome>, anyhow::Error> {
        let outcome = self.call(|api, scope| api.init(scope))?;
        Ok(Output::Render(outcome))
    }

    pub fn init_link(&self, target: &str) -> Result<Output<InitializationOutcome>, anyhow::Error> {
        let target_path = std::path::PathBuf::from(target);
        let local_padz = &self.state.local_padz_dir;
        let outcome = self.call(|api, _scope| api.init_link(local_padz, &target_path))?;
        Ok(Output::Render(outcome))
    }

    pub fn init_unlink(&self) -> Result<Output<InitializationOutcome>, anyhow::Error> {
        let local_padz = &self.state.local_padz_dir;
        let outcome = self.call(|api, _scope| api.init_unlink(local_padz))?;
        Ok(Output::Render(outcome))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn export_pads(
        &self,
        indexes: &[String],
        single_file: Option<&str>,
        json: bool,
        with_metadata: bool,
        nesting: NestingMode,
    ) -> Result<Output<padzapp::commands::export::ExportReport>, anyhow::Error> {
        let result = if let Some(title) = single_file {
            self.call(|api, scope| api.export_pads_single_file(scope, indexes, title, nesting))?
        } else if json {
            self.call(|api, scope| api.export_pads_json(scope, indexes, nesting))?
        } else {
            self.call(|api, scope| api.export_pads(scope, indexes, nesting, with_metadata))?
        };
        Ok(match result {
            // An empty selection never enters the artifact path: Standout only
            // merges its receipt after a write, so `export.jinja` keys the empty
            // message on the receipt being absent. `exported: 0` states the fact.
            padzapp::commands::export::ExportOutcome::Empty { format } => {
                Output::Render(padzapp::commands::export::ExportReport {
                    format,
                    exported: 0,
                    warnings: Vec::new(),
                })
            }
            // The core report rides along as the artifact's semantic report;
            // Standout owns the write and merges its own receipt afterwards.
            padzapp::commands::export::ExportOutcome::Artifact(artifact) => Output::Artifact(
                Artifact::new(artifact.bytes)
                    .suggest_destination(artifact.suggested_filename)
                    .with_report(artifact.report),
            ),
        })
    }

    /// Return the core semantic import report for the CLI to render directly.
    pub fn import_pads(
        &self,
        paths: Vec<std::path::PathBuf>,
    ) -> Result<Output<padzapp::commands::import::ImportReport>, anyhow::Error> {
        let extensions = &self.state.import_extensions.0;
        let report = self.call(move |api, scope| api.import_pads(scope, paths, extensions))?;
        Ok(Output::Render(report))
    }

    pub fn transfer_pads(
        &self,
        indexes: &[String],
        to: Option<&str>,
        from: Option<&str>,
        mode: padzapp::commands::transfer::TransferMode,
    ) -> Result<Output<padzapp::commands::transfer::TransferReport>, anyhow::Error> {
        match (to, from) {
            (Some(dest), None) => {
                let path = std::path::PathBuf::from(dest);
                let result =
                    self.call(|api, scope| api.transfer_pads_to(scope, indexes, &path, mode))?;
                Ok(Output::Render(result))
            }
            (None, Some(src)) => {
                let path = std::path::PathBuf::from(src);
                let result =
                    self.call(|api, scope| api.transfer_pads_from(scope, indexes, &path, mode))?;
                Ok(Output::Render(result))
            }
            (Some(_), Some(_)) => Err(anyhow::anyhow!("--to and --from are mutually exclusive")),
            (None, None) => Err(anyhow::anyhow!("exactly one of --to or --from is required")),
        }
    }

    // --- Tags subcommand operations ---

    pub fn list_tags(&self) -> Result<Output<TagCatalogOutcome>, anyhow::Error> {
        let outcome = self.call(|api, scope| api.list_tags(scope))?;
        Ok(Output::Render(outcome))
    }

    pub fn delete_tag(&self, name: &str) -> Result<Output<TagRegistryOutcome>, anyhow::Error> {
        let outcome = self.call(|api, scope| api.delete_tag(scope, name))?;
        Ok(Output::Render(outcome))
    }

    pub fn rename_tag(
        &self,
        old_name: &str,
        new_name: &str,
    ) -> Result<Output<TagRegistryOutcome>, anyhow::Error> {
        let outcome = self.call(|api, scope| api.rename_tag(scope, old_name, new_name))?;
        Ok(Output::Render(outcome))
    }

    // --- List operations ---

    #[allow(clippy::too_many_arguments)]
    pub fn list_pads(
        &self,
        filter: PadFilter,
        peek: bool,
        show_deleted_help: bool,
        show_all_sections: bool,
        ids: &[String],
        show_uuid: bool,
        show_status: bool,
    ) -> Result<Output<Listing>, anyhow::Error> {
        let filtered = filter.search_term.is_some()
            || filter.todo_status.is_some()
            || filter.tags.is_some()
            || !ids.is_empty();
        let result = self.call(|api, scope| api.get_pads(scope, filter, ids))?;
        Ok(Output::Render(Listing {
            pads: result.listed_pads,
            request: ListRequest {
                peek,
                uuid: show_uuid,
                status: self.state.wants_status(show_status),
                filtered,
                deleted_help: show_deleted_help,
                sections: show_all_sections,
            },
        }))
    }

    // --- View operations ---

    pub fn view_pads(
        &self,
        indexes: &[String],
        show_uuid: bool,
        nesting: NestingMode,
    ) -> Result<Output<PadContentResult>, anyhow::Error> {
        let result = self.call(|api, scope| api.view_pads(scope, indexes, nesting))?;

        let pads: Vec<PadContent> = result
            .listed_pads
            .iter()
            .enumerate()
            .map(|(i, dp)| {
                let depth = result.listed_depths.get(i).copied().unwrap_or(0);
                // Extract body (content minus title) to avoid double-title in output
                let body = extract_title_and_body(&dp.pad.content)
                    .map(|(_, b)| b)
                    .unwrap_or_default();

                PadContent {
                    title: dp.pad.metadata.title.clone(),
                    content: body,
                    depth,
                    uuid: show_uuid.then(|| dp.pad.metadata.id.to_string()),
                }
            })
            .collect();
        let view = PadContentResult { pads, nesting };

        // `view` copies only the selected roots, in display order. Build one
        // payload and perform one CLI-owned write so multiple selectors do not
        // overwrite one another and rendered/structured output never becomes the
        // clipboard source.
        let clipboard_text = view_clipboard_text(&view);
        if !clipboard_text.is_empty() {
            self.state.copy_to_clipboard(&clipboard_text);
        }

        Ok(Output::Render(view))
    }

    // --- Copy operations ---

    pub fn copy_pads(
        &self,
        indexes: &[String],
        nesting: NestingMode,
    ) -> Result<Output<CopyView>, anyhow::Error> {
        let result = self.call(|api, scope| api.view_pads(scope, indexes, nesting))?;

        let indent_per_level: usize = match nesting {
            NestingMode::Indented => 4,
            _ => 0,
        };

        // Build clipboard text: root pads separated by ---, children appended under parent
        let mut clipboard_text = String::new();
        for (i, dp) in result.listed_pads.iter().enumerate() {
            let depth = result.listed_depths.get(i).copied().unwrap_or(0);
            let indent = " ".repeat(depth * indent_per_level);
            let body = extract_title_and_body(&dp.pad.content)
                .map(|(_, b)| b)
                .unwrap_or_default();

            // --- separator only between root-level pads (not between parent and child)
            if depth == 0 && !clipboard_text.is_empty() {
                clipboard_text.push_str("\n---\n\n");
            } else if depth > 0 {
                clipboard_text.push_str("\n\n");
            }

            if indent.is_empty() {
                clipboard_text.push_str(&format_for_clipboard(&dp.pad.metadata.title, &body));
            } else {
                clipboard_text.push_str(&format_for_clipboard(
                    &format!("{}{}", indent, dp.pad.metadata.title),
                    &indent_lines(&body, &indent),
                ));
            }
        }

        self.state.copy_to_clipboard(&clipboard_text);

        // Report using only the root-level (depth 0) pad titles. These are the
        // selected roots; descendants belong in the nested payload but are not
        // additional selections.
        let titles: Vec<String> = result
            .listed_pads
            .iter()
            .enumerate()
            .filter(|(i, _)| result.listed_depths.get(*i).copied().unwrap_or(0) == 0)
            .map(|(_, dp)| dp.pad.metadata.title.clone())
            .collect();

        Ok(Output::Render(CopyView {
            root_pad_count: titles.len(),
            titles,
        }))
    }
}

/// Clipboard payload for `view`, derived only from its semantic result.
///
/// Child rows are excluded to preserve `view`'s established clipboard contract;
/// selected roots are joined in their canonical display order with the same
/// separator used by human multi-pad output.
fn view_clipboard_text(view: &PadContentResult) -> String {
    view.pads
        .iter()
        .filter(|pad| pad.depth == 0)
        .map(|pad| format_for_clipboard(&pad.title, &pad.content))
        .collect::<Vec<_>>()
        .join("\n---\n\n")
}

/// Get a scoped API accessor from the command context
fn api(ctx: &CommandContext) -> ScopedApi<'_> {
    ScopedApi {
        state: get_state(ctx),
    }
}

/// Parse --flat/--tree/--indented flags into a NestingMode.
/// Default is Tree when none specified.
fn parse_nesting_mode(flat: bool, _tree: bool, indented: bool) -> NestingMode {
    if flat {
        NestingMode::Flat
    } else if indented {
        NestingMode::Indented
    } else {
        // --tree or default
        NestingMode::Tree
    }
}

/// Helper to copy pad content to clipboard (from content string)
fn copy_content_to_clipboard(state: &AppState, content: &str) {
    if let Some((title, body)) = extract_title_and_body(content) {
        let clipboard_text = format_for_clipboard(&title, &body);
        state.copy_to_clipboard(&clipboard_text);
    }
}

/// Indent each non-empty line with the given prefix.
fn indent_lines(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| {
            if line.is_empty() {
                String::new()
            } else {
                format!("{}{}", prefix, line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Split a list of args into index selectors and trailing content words.
///
/// Index patterns are: digits, pN, dN, N.N, N-N, pN-pN etc.
/// Once an arg fails to parse as an index, everything from that point is content.
///
/// Shared with [`crate::cli::input`], whose edit source decides whether the
/// trailing words make a quick-edit and so must split them the same way.
pub(crate) fn split_indexes_and_content(args: &[String]) -> (Vec<String>, Vec<String>) {
    use padzapp::index::parse_index_or_range;

    let mut indexes = Vec::new();
    let mut content = Vec::new();
    let mut past_indexes = false;

    for arg in args {
        if past_indexes {
            content.push(arg.clone());
        } else if parse_index_or_range(arg).is_ok() {
            indexes.push(arg.clone());
        } else {
            past_indexes = true;
            content.push(arg.clone());
        }
    }

    // If no indexes were found, all args are likely a title search term.
    // Return them as indexes so parse_selectors can apply its title fallback.
    if indexes.is_empty() && !content.is_empty() {
        return (std::mem::take(&mut content), vec![]);
    }

    (indexes, content)
}

// =============================================================================
// Core commands
// =============================================================================

/// Create a pad.
///
/// Where the text comes from — args, piped stdin, or the editor — is *not*
/// decided here: `cli::input`'s chain resolves it before dispatch and this
/// handler matches on the resulting [`RequestContent`]. The `editor` /
/// `no_editor` flags are part of that chain's availability rules, so they are
/// not read here either. Both paths return the same core [`Modification`] the rest
/// of the family does (rendered by `modification_result.jinja` with `action =
/// "create"`): a success carries the created pad, and empty piped/editor content
/// carries no pads, which the template reads as the aborted-empty-content case.
#[handler]
pub fn create(
    #[ctx] ctx: &CommandContext,
    #[arg] inside: Option<String>,
    #[arg] format: Option<String>,
    #[arg] title: Vec<String>,
) -> Result<Output<Modification>, anyhow::Error> {
    let state = get_state(ctx);
    let content = ctx.input::<RequestContent>(CREATE_CONTENT)?;
    let title_arg = if title.is_empty() {
        None
    } else {
        Some(title.join(" "))
    };
    let inside = inside.as_deref();
    let format_ref = format.as_deref();

    // Helper to call create_pad with or without format override
    fn do_create(
        state: &AppState,
        title: String,
        content: String,
        inside: Option<&str>,
        format: Option<&str>,
    ) -> std::result::Result<padzapp::commands::CmdResult, anyhow::Error> {
        state.with_api(|api| {
            if let Some(fmt) = format {
                api.create_pad_with_format(state.scope, title, content, inside, fmt)
                    .map_err(to_anyhow)
            } else {
                api.create_pad(state.scope, title, content, inside)
                    .map_err(to_anyhow)
            }
        })
    }

    let result = match content {
        // Quick-create: args used directly, no editor. The chain already joined
        // the args and expanded literal `\n`.
        RequestContent::Direct(expanded) => {
            let (title, body) =
                extract_title_and_body(expanded).unwrap_or_else(|| (String::new(), String::new()));
            let result = do_create(state, title.clone(), body.clone(), inside, format_ref)?;
            // Propagate status now — content is non-empty, safe from reconciliation
            let parent_id = result.affected_pads[0].pad.metadata.parent_id;
            state.with_api(|api| api.propagate_status(state.scope, parent_id))?;
            // Copy to clipboard
            let clipboard_text = format_for_clipboard(&title, &body);
            state.copy_to_clipboard(&clipboard_text);
            result
        }

        // Piped content from stdin.
        RequestContent::Piped(raw) => {
            let parsed = padzapp::editor::EditorContent::from_buffer(raw);
            // Determine title: title_arg override > parsed title > empty
            let final_title = match (&title_arg, parsed.title.is_empty()) {
                (Some(t), _) => t.clone(),  // CLI title always wins
                (_, false) => parsed.title, // parsed has title, no CLI override
                (None, true) => String::new(),
            };
            let result = do_create(state, final_title, parsed.content, inside, format_ref)?;
            // Propagate status now — content is non-empty, safe from reconciliation
            let parent_id = result.affected_pads[0].pad.metadata.parent_id;
            state.with_api(|api| api.propagate_status(state.scope, parent_id))?;
            // Copy to clipboard
            copy_content_to_clipboard(state, raw);
            result
        }

        // An empty pipe is an abort: no pad, no editor.
        RequestContent::PipedEmpty => return Ok(aborted_create(ctx)),

        // Interactive: create pad first, then open real file in editor.
        //
        // This arm is why the chain stops at a decision rather than resolving a
        // string through standout's `EditorSource`: the editor runs against the
        // pad's real file in `.padz/`, and a failed launch must delete the pad
        // that was created to hold it.
        RequestContent::Editor => {
            let initial_title = title_arg.clone().unwrap_or_default();
            let create_result = do_create(state, initial_title, String::new(), inside, format_ref)?;
            let pad_path = create_result.pad_paths[0].clone();
            let pad_id = create_result.affected_pads[0].pad.metadata.id;

            // Open editor on the real pad file in .padz/
            if let Err(e) = crate::cli::editor::open_in_editor(&pad_path) {
                // Editor failed - clean up the pad
                let _ = state.with_api(|api| api.remove_pad(state.scope, pad_id));
                return Err(to_anyhow(e));
            }

            // Refresh pad from disk (re-reads content, updates title)
            match state.with_api(|api| api.refresh_pad(state.scope, &pad_id).map_err(to_anyhow))? {
                Some(pad) => {
                    // Propagate status now — pad has real content after editor,
                    // safe from reconciliation deleting the empty file.
                    let parent_id = pad.metadata.parent_id;
                    state.with_api(|api| {
                        api.propagate_status(state.scope, parent_id)
                            .map_err(to_anyhow)
                    })?;
                    // Copy to clipboard
                    copy_content_to_clipboard(state, &pad.content);
                    // Build result
                    let display_pad = padzapp::index::DisplayPad {
                        pad,
                        index: padzapp::index::DisplayIndex::Regular(1),
                        matches: None,
                        children: Vec::new(),
                    };
                    CmdResult {
                        affected_pads: vec![display_pad],
                        ..Default::default()
                    }
                }
                None => {
                    // Empty file - user aborted
                    return Ok(aborted_create(ctx));
                }
            }
        }
    };

    Ok(Output::Render(api(ctx).modification_result(
        ModificationAction::Create,
        result,
        false,
    )))
}

/// The result of a `create` the user abandoned by supplying no content.
///
/// No pad was created, so the outcome is a `create` [`Modification`] with no
/// affected pads — the shape `modification_result.jinja` renders as the
/// aborted-empty-content warning. Keeping it a `Modification` (rather than a
/// bespoke abort projection) is what lets create share the family's core outcome.
fn aborted_create(ctx: &CommandContext) -> Output<Modification> {
    Output::Render(api(ctx).modification_result(
        ModificationAction::Create,
        CmdResult::default(),
        false,
    ))
}

/// List all pads with optional filtering.
///
/// Uses `#[handler]` macro - requires `#[dispatch(pure)]` in setup.rs.
#[allow(clippy::too_many_arguments)]
#[handler]
pub fn list(
    #[ctx] ctx: &CommandContext,
    #[arg] ids: Vec<String>,
    #[arg] search: Option<String>,
    #[flag] deleted: bool,
    #[flag] archived: bool,
    #[flag] all: bool,
    #[flag] peek: bool,
    #[flag] planned: bool,
    #[flag] completed: bool,
    #[flag(name = "in_progress")] in_progress: bool,
    #[arg] tags: Vec<String>,
    #[flag] uuid: bool,
    #[flag(name = "show_status")] show_status: bool,
) -> Result<Output<Listing>, anyhow::Error> {
    let todo_status = if planned {
        Some(TodoStatus::Planned)
    } else if completed {
        Some(TodoStatus::Done)
    } else if in_progress {
        Some(TodoStatus::InProgress)
    } else {
        None
    };

    let filter = PadFilter {
        status: if all {
            PadStatusFilter::All
        } else if deleted {
            PadStatusFilter::Deleted
        } else if archived {
            PadStatusFilter::Archived
        } else {
            PadStatusFilter::Active
        },
        search_term: search,
        todo_status,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };

    api(ctx).list_pads(
        filter,
        peek,
        deleted || archived,
        all,
        &ids,
        uuid,
        show_status,
    )
}

#[handler]
pub fn peek(
    #[ctx] ctx: &CommandContext,
    #[arg] ids: Vec<String>,
    #[arg] tags: Vec<String>,
    #[flag] uuid: bool,
) -> Result<Output<Listing>, anyhow::Error> {
    let filter = PadFilter {
        status: PadStatusFilter::Active,
        search_term: None,
        todo_status: None,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };
    api(ctx).list_pads(filter, true, false, false, &ids, uuid, false)
}

#[allow(clippy::too_many_arguments)]
#[handler]
pub fn search(
    #[ctx] ctx: &CommandContext,
    #[arg] term: String,
    #[flag] deleted: bool,
    #[flag] archived: bool,
    #[flag] all: bool,
    #[flag] completed: bool,
    #[arg] tags: Vec<String>,
    #[flag] uuid: bool,
) -> Result<Output<Listing>, anyhow::Error> {
    let filter = PadFilter {
        status: if all {
            PadStatusFilter::All
        } else if deleted {
            PadStatusFilter::Deleted
        } else if archived {
            PadStatusFilter::Archived
        } else {
            PadStatusFilter::Active
        },
        search_term: Some(term),
        todo_status: if completed {
            Some(TodoStatus::Done)
        } else {
            None
        },
        tags: if tags.is_empty() { None } else { Some(tags) },
    };

    api(ctx).list_pads(filter, false, deleted || archived, all, &[], uuid, false)
}

// =============================================================================
// Pad operations
// =============================================================================

#[handler]
pub fn view(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag(name = "peek")] _peek: bool, // Reserved for future use
    #[flag] uuid: bool,
    #[flag] flat: bool,
    #[flag] tree: bool,
    #[flag] indented: bool,
) -> Result<Output<PadContentResult>, anyhow::Error> {
    let nesting = parse_nesting_mode(flat, tree, indented);
    api(ctx).view_pads(&indexes, uuid, nesting)
}

/// Copy selected pads to one ordered clipboard payload and return root-selection facts.
#[handler]
pub fn copy(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag(name = "peek")] _peek: bool, // Reserved for future use
    #[flag] flat: bool,
    #[flag] tree: bool,
    #[flag] indented: bool,
) -> Result<Output<CopyView>, anyhow::Error> {
    let nesting = parse_nesting_mode(flat, tree, indented);
    api(ctx).copy_pads(&indexes, nesting)
}

/// Edit a pad.
///
/// Like [`create`], the content source is resolved before dispatch by
/// `cli::input`'s chain; this handler only splits the index selectors out of
/// the positional args and acts on the resolved decision.
#[handler]
pub fn edit(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Modification>, anyhow::Error> {
    let state = get_state(ctx);
    let content = ctx.input::<RequestContent>(EDIT_CONTENT)?;

    // Split args into index selectors and trailing content words.
    // Index patterns: digits, pN, dN, N.N, N-N, pN-pN, etc.
    // Only the selectors matter here; the trailing words already reached the
    // input chain, which decided whether they are a quick-edit.
    let (index_args, _) = split_indexes_and_content(&indexes);

    if index_args.is_empty() {
        return Err(anyhow::anyhow!("No pad index provided"));
    }

    // Writes `raw` to every selected pad.
    let update = |raw: &str| -> Result<CmdResult, anyhow::Error> {
        state.with_api(|api| {
            api.update_pads_from_content(state.scope, &index_args, raw)
                .map_err(to_anyhow)
        })
    };

    match content {
        // Quick-edit (todos mode, inline words): the raw text is copied as typed.
        RequestContent::Direct(raw) => {
            let result = update(raw)?;
            state.copy_to_clipboard(raw);
            return Ok(Output::Render(api(ctx).modification_result(
                ModificationAction::Update,
                result,
                false,
            )));
        }

        // Piped content is copied in the pad's title/body shape.
        RequestContent::Piped(raw) => {
            let result = update(raw)?;
            copy_content_to_clipboard(state, raw);
            return Ok(Output::Render(api(ctx).modification_result(
                ModificationAction::Update,
                result,
                false,
            )));
        }

        // An empty pipe aborts the edit outright — unlike create, which reports
        // the abort as a warning, this has always been an error.
        RequestContent::PipedEmpty => return Err(anyhow::anyhow!("Aborted: empty content")),

        // Fall through to the interactive editor below.
        RequestContent::Editor => {}
    }

    // Interactive editor: open real pad file
    let view_result = state.with_api(|api| {
        api.view_pads(
            state.scope,
            &index_args,
            padzapp::commands::NestingMode::Flat,
        )
        .map_err(to_anyhow)
    })?;

    let pad = view_result
        .listed_pads
        .first()
        .ok_or_else(|| anyhow::anyhow!("No pad found"))?;
    let pad_id = pad.pad.metadata.id;
    let display_index = pad.index.clone();
    let display_path = state.with_api(|api| {
        api.display_path_by_id(state.scope, pad_id)
            .map_err(to_anyhow)
    })?;

    let pad_path =
        state.with_api(|api| api.get_path_by_id(state.scope, pad_id).map_err(to_anyhow))?;

    // Open editor on the real pad file in .padz/
    crate::cli::editor::open_in_editor(&pad_path)?;

    // Refresh pad from disk (re-reads content, updates title)
    match state.with_api(|api| api.refresh_pad(state.scope, &pad_id).map_err(to_anyhow))? {
        Some(pad) => {
            copy_content_to_clipboard(state, &pad.content);
            let title = pad.metadata.title.clone();

            let display_pad = padzapp::index::DisplayPad {
                pad,
                index: display_index,
                matches: None,
                children: Vec::new(),
            };
            let result = CmdResult {
                affected_pads: vec![display_pad],
                outcomes: vec![CmdOutcome::Updated {
                    path: display_path,
                    title,
                    update_kind: UpdateKind::Refresh,
                }],
                ..Default::default()
            };
            Ok(Output::Render(api(ctx).modification_result(
                ModificationAction::Update,
                result,
                false,
            )))
        }
        None => {
            // User emptied the file
            Ok(Output::<Modification>::Silent)
        }
    }
}

#[handler]
pub fn delete(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag] completed: bool,
) -> Result<Output<Modification>, anyhow::Error> {
    if completed {
        api(ctx).delete_completed_pads()
    } else {
        api(ctx).delete_pads(&indexes)
    }
}

#[handler]
pub fn restore(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Modification>, anyhow::Error> {
    api(ctx).restore_pads(&indexes)
}

#[handler]
pub fn archive(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Modification>, anyhow::Error> {
    api(ctx).archive_pads(&indexes)
}

#[handler]
pub fn unarchive(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Modification>, anyhow::Error> {
    api(ctx).unarchive_pads(&indexes)
}

/// Pin pads to the top of the list.
#[handler]
pub fn pin(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Modification>, anyhow::Error> {
    api(ctx).pin_pads(&indexes)
}

#[handler]
pub fn unpin(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Modification>, anyhow::Error> {
    api(ctx).unpin_pads(&indexes)
}

#[handler]
pub fn move_pads(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag] root: bool,
) -> Result<Output<Modification>, anyhow::Error> {
    api(ctx).move_pads(&indexes, root)
}

/// Returns the file path of each selected pad.
#[handler]
pub fn path(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<PathView>, anyhow::Error> {
    let state = get_state(ctx);
    let result = state.with_api(|api| api.pad_paths(state.scope, &indexes).map_err(to_anyhow))?;

    Ok(Output::Render(PathView {
        paths: result
            .pad_paths
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
    }))
}

/// Maps each selected domain UUID into the CLI's stable textual UUID result.
#[handler]
pub fn uuid(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<UuidView>, anyhow::Error> {
    let state = get_state(ctx);
    let result = state.with_api(|api| api.pad_uuids(state.scope, &indexes).map_err(to_anyhow))?;

    Ok(Output::Render(UuidView {
        uuids: result.into_iter().map(|uuid| uuid.to_string()).collect(),
    }))
}

#[handler]
pub fn complete(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Modification>, anyhow::Error> {
    api(ctx).complete_pads(&indexes)
}

#[handler]
pub fn reopen(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Modification>, anyhow::Error> {
    api(ctx).reopen_pads(&indexes)
}

// =============================================================================
// Data operations
// =============================================================================

#[handler]
pub fn purge(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag] yes: bool,
    #[flag] recursive: bool,
) -> Result<Output<PurgeOutcome>, anyhow::Error> {
    api(ctx).purge_pads(&indexes, yes, recursive)
}

#[allow(clippy::too_many_arguments)]
#[handler]
pub fn export(
    #[ctx] ctx: &CommandContext,
    #[arg(name = "single_file")] single_file: Option<String>,
    #[flag] json: bool,
    #[flag(name = "with_metadata")] with_metadata: bool,
    #[arg] indexes: Vec<String>,
    #[flag] flat: bool,
    #[flag] tree: bool,
    #[flag] indented: bool,
) -> Result<Output<padzapp::commands::export::ExportReport>, anyhow::Error> {
    let nesting = parse_nesting_mode(flat, tree, indented);
    api(ctx).export_pads(
        &indexes,
        single_file.as_deref(),
        json,
        with_metadata,
        nesting,
    )
}

/// Import requested paths and return mode-independent semantic facts.
#[handler]
pub fn import(
    #[ctx] ctx: &CommandContext,
    #[arg] paths: Vec<String>,
) -> Result<Output<padzapp::commands::import::ImportReport>, anyhow::Error> {
    let paths: Vec<std::path::PathBuf> = paths.into_iter().map(std::path::PathBuf::from).collect();
    api(ctx).import_pads(paths)
}

#[handler]
pub fn clone(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[arg] to: Option<String>,
    #[arg] from: Option<String>,
) -> Result<Output<padzapp::commands::transfer::TransferReport>, anyhow::Error> {
    api(ctx).transfer_pads(
        &indexes,
        to.as_deref(),
        from.as_deref(),
        padzapp::commands::transfer::TransferMode::Clone,
    )
}

#[handler]
pub fn migrate(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[arg] to: Option<String>,
    #[arg] from: Option<String>,
) -> Result<Output<padzapp::commands::transfer::TransferReport>, anyhow::Error> {
    api(ctx).transfer_pads(
        &indexes,
        to.as_deref(),
        from.as_deref(),
        padzapp::commands::transfer::TransferMode::Migrate,
    )
}

// =============================================================================
// Misc commands
// =============================================================================

#[handler]
pub fn doctor(#[ctx] ctx: &CommandContext) -> Result<Output<DoctorOutcome>, anyhow::Error> {
    api(ctx).doctor()
}

#[handler]
pub fn init(
    #[ctx] ctx: &CommandContext,
    #[arg] link: Option<String>,
    #[flag] unlink: bool,
) -> Result<Output<InitializationOutcome>, anyhow::Error> {
    if let Some(target) = link {
        api(ctx).init_link(&target)
    } else if unlink {
        api(ctx).init_unlink()
    } else {
        api(ctx).init()
    }
}

// =============================================================================
// Tag subcommand handlers
// =============================================================================

/// Split positional args into pad selectors and tag names.
///
/// Leading args that parse as index selectors (via `parse_index_or_range`) are pad IDs.
/// Once one fails, everything from that point on is a tag name.
/// Requires at least 1 selector and at least 1 tag.
fn split_indexes_and_tags(args: &[String]) -> Result<(Vec<String>, Vec<String>), anyhow::Error> {
    use padzapp::index::parse_index_or_range;

    let mut indexes = Vec::new();
    let mut tags = Vec::new();
    let mut past_indexes = false;

    for arg in args {
        if past_indexes {
            tags.push(arg.clone());
        } else if parse_index_or_range(arg).is_ok() {
            indexes.push(arg.clone());
        } else {
            past_indexes = true;
            tags.push(arg.clone());
        }
    }

    if indexes.is_empty() {
        return Err(anyhow::anyhow!(
            "No pad selectors provided. Usage: padz tag add <id>... <tag>..."
        ));
    }
    if tags.is_empty() {
        return Err(anyhow::anyhow!(
            "No tag names provided. Usage: padz tag add <id>... <tag>..."
        ));
    }

    Ok((indexes, tags))
}

pub mod tag {
    use super::*;

    #[handler]
    pub fn add(
        #[ctx] ctx: &CommandContext,
        #[arg] args: Vec<String>,
    ) -> Result<Output<TaggingResult>, anyhow::Error> {
        let (indexes, tags) = split_indexes_and_tags(&args)?;
        let state = get_state(ctx);
        let result = state.with_api(|api| {
            api.add_tags_to_pads(state.scope, &indexes, &tags)
                .map_err(to_anyhow)
        })?;
        Ok(Output::Render(result))
    }

    #[handler]
    pub fn remove(
        #[ctx] ctx: &CommandContext,
        #[arg] args: Vec<String>,
    ) -> Result<Output<TaggingResult>, anyhow::Error> {
        let (indexes, tags) = split_indexes_and_tags(&args)?;
        let state = get_state(ctx);
        let result = state.with_api(|api| {
            api.remove_tags_from_pads(state.scope, &indexes, &tags)
                .map_err(to_anyhow)
        })?;
        Ok(Output::Render(result))
    }

    #[handler]
    pub fn rename(
        #[ctx] ctx: &CommandContext,
        #[arg(name = "old_name")] old_name: String,
        #[arg(name = "new_name")] new_name: String,
    ) -> Result<Output<TagRegistryOutcome>, anyhow::Error> {
        api(ctx).rename_tag(&old_name, &new_name)
    }

    #[handler]
    pub fn delete(
        #[ctx] ctx: &CommandContext,
        #[arg] name: String,
    ) -> Result<Output<TagRegistryOutcome>, anyhow::Error> {
        api(ctx).delete_tag(&name)
    }

    #[handler]
    pub fn list(
        #[ctx] ctx: &CommandContext,
        #[arg] ids: Vec<String>,
    ) -> Result<Output<TagCatalogOutcome>, anyhow::Error> {
        if ids.is_empty() {
            api(ctx).list_tags()
        } else {
            let state = get_state(ctx);
            let outcome =
                state.with_api(|api| api.list_pad_tags(state.scope, &ids).map_err(to_anyhow))?;
            Ok(Output::Render(outcome))
        }
    }
}

#[cfg(test)]
mod tests {
    //! Direct typed-handler tests.
    //!
    //! These call the typed function the `#[handler]` macro preserves — no
    //! `ArgMatches`, no dispatch, no renderer. That is the whole point: a handler's
    //! value is the same object whatever `--output` the user passed, so asserting on
    //! it here is asserting on what both the human and the structured path receive.

    use super::*;
    use padzapp::index::DisplayIndex;
    use padzapp::init::initialize;
    use standout_dispatch::Extensions;
    use std::rc::Rc;
    use tempfile::TempDir;

    /// A project-scoped store in a temp dir, wired into a CommandContext.
    ///
    /// Uses `data_override` for the project store and an explicit [`PadzEnv`]
    /// pointing global inside the same temp dir, so the fixture reads no
    /// process environment and can never touch the developer's real global
    /// store.
    struct TestApp {
        _temp: TempDir,
        ctx: CommandContext,
    }

    impl TestApp {
        fn new(mode: PadzMode) -> Self {
            let temp = TempDir::new().unwrap();
            let root = temp.path().to_path_buf();
            let env = padzapp::init::PadzEnv {
                global_data_dir: root.join("global-data"),
                home_dir: None,
            };
            let padz_ctx = initialize(&env, &root, false, Some(root.clone()), true).unwrap();

            let state = AppState::new(
                padz_ctx.api,
                padz_ctx.scope,
                vec!["txt".to_string(), "md".to_string()],
                mode,
                root.join(".padz"),
            );

            let mut app_state = Extensions::new();
            app_state.insert(state);

            Self {
                _temp: temp,
                ctx: CommandContext::new(vec![], Rc::new(app_state)),
            }
        }

        /// Seeds a pad straight through the API, bypassing the create handler (which
        /// would want an editor).
        fn seed(&self, title: &str, body: &str) {
            let state = get_state(&self.ctx);
            state
                .with_api(|api| api.create_pad(state.scope, title.into(), body.into(), None))
                .unwrap();
        }
    }

    fn rendered<T>(output: Output<T>) -> T
    where
        T: serde::Serialize,
    {
        match output {
            Output::Render(data) => data,
            _ => panic!("expected Output::Render"),
        }
    }

    // --- list -----------------------------------------------------------------

    #[test]
    fn list_returns_typed_pads_with_display_indexes() {
        let app = TestApp::new(PadzMode::Notes);
        app.seed("First", "body");
        app.seed("Second", "body");

        let result = rendered(
            list(
                &app.ctx,
                vec![],
                None,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                vec![],
                false,
                false,
            )
            .unwrap(),
        );

        assert_eq!(result.pads.len(), 2);
        // Canonical display identifiers come from index_pads and are preserved as-is.
        assert!(matches!(result.pads[0].index, DisplayIndex::Regular(1)));
        assert!(matches!(result.pads[1].index, DisplayIndex::Regular(2)));
        assert_eq!(result.request, ListRequest::default());
    }

    #[test]
    fn list_in_todos_mode_requests_status_icons() {
        let app = TestApp::new(PadzMode::Todos);
        app.seed("Todo", "body");

        let result = rendered(
            list(
                &app.ctx,
                vec![],
                None,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                vec![],
                false,
                false,
            )
            .unwrap(),
        );

        // Todos mode asks for status icons implicitly; notes mode does not.
        assert!(result.request.status);
    }

    #[test]
    fn list_records_the_display_request() {
        let app = TestApp::new(PadzMode::Notes);
        app.seed("Only", "body");

        let result = rendered(
            list(
                &app.ctx,
                vec![],
                None,
                false,
                false,
                false,
                true, // peek
                false,
                false,
                false,
                vec![],
                true,  // uuid
                false, // show_status
            )
            .unwrap(),
        );

        assert!(result.request.peek);
        assert!(result.request.uuid);
        assert!(!result.request.filtered);
    }

    #[test]
    fn search_marks_the_listing_filtered() {
        let app = TestApp::new(PadzMode::Notes);
        app.seed("Alpha", "body");
        app.seed("Beta", "body");

        let result = rendered(
            search(
                &app.ctx,
                "Alpha".to_string(),
                false,
                false,
                false,
                false,
                vec![],
                false,
            )
            .unwrap(),
        );

        // An empty filtered listing means "nothing matched", not "no pads yet".
        assert!(result.request.filtered);
        assert_eq!(result.pads.len(), 1);
        assert_eq!(result.pads[0].pad.metadata.title, "Alpha");
    }

    // --- view -----------------------------------------------------------------

    #[test]
    fn view_returns_title_and_body_without_uuid_by_default() {
        let app = TestApp::new(PadzMode::Notes);
        app.seed("Viewed", "the body");

        let result = rendered(
            view(
                &app.ctx,
                vec!["1".into()],
                false,
                false,
                false,
                false,
                false,
            )
            .unwrap(),
        );

        assert_eq!(result.pads.len(), 1);
        assert_eq!(result.pads[0].title, "Viewed");
        assert!(result.pads[0].content.contains("the body"));
        assert_eq!(result.pads[0].depth, 0);
        assert!(result.pads[0].uuid.is_none());
    }

    #[test]
    fn view_includes_uuid_when_requested() {
        let app = TestApp::new(PadzMode::Notes);
        app.seed("Viewed", "the body");

        // view's flags are (peek, uuid, flat, tree, indented).
        let result =
            rendered(view(&app.ctx, vec!["1".into()], false, true, false, false, false).unwrap());

        assert!(result.pads[0].uuid.is_some());
    }

    #[test]
    fn view_clipboard_payload_joins_roots_once_and_excludes_children() {
        let view = PadContentResult {
            nesting: NestingMode::Tree,
            pads: vec![
                PadContent {
                    title: "First".to_string(),
                    content: "one".to_string(),
                    depth: 0,
                    uuid: None,
                },
                PadContent {
                    title: "Child".to_string(),
                    content: "nested".to_string(),
                    depth: 1,
                    uuid: None,
                },
                PadContent {
                    title: "Second".to_string(),
                    content: "two".to_string(),
                    depth: 0,
                    uuid: None,
                },
            ],
        };

        assert_eq!(
            view_clipboard_text(&view),
            "First\n\none\n---\n\nSecond\n\ntwo"
        );
    }

    // --- mutation -------------------------------------------------------------

    #[test]
    fn pin_returns_the_action_and_the_affected_pad() {
        let app = TestApp::new(PadzMode::Notes);
        app.seed("Pin me", "body");

        let result = rendered(pin(&app.ctx, vec!["1".into()]).unwrap());

        assert_eq!(result.action, ModificationAction::Pin);
        assert_eq!(result.pads.len(), 1);
        assert_eq!(result.pads[0].pad.metadata.title, "Pin me");
        assert!(result.pads[0].pad.metadata.is_pinned);
    }

    #[test]
    fn complete_requests_status_icons_even_in_notes_mode() {
        let app = TestApp::new(PadzMode::Notes);
        app.seed("Finish me", "body");

        let result = rendered(complete(&app.ctx, vec!["1".into()]).unwrap());

        assert_eq!(result.action, ModificationAction::Complete);
        // A command that changes status always shows it, whatever the mode.
        assert!(result.request.status);
    }

    #[test]
    fn delete_completed_with_nothing_to_delete_reports_a_semantic_no_op() {
        let app = TestApp::new(PadzMode::Todos);
        app.seed("Still open", "body");

        let result = rendered(delete(&app.ctx, vec![], true).unwrap());

        assert!(result.pads.is_empty());
        assert_eq!(
            result.notices,
            vec![padzapp::commands::CmdNotice::NoCompletedPads]
        );
    }

    // --- path / uuid ----------------------------------------------------------

    #[test]
    fn path_returns_the_pad_file_path_instead_of_printing_it() {
        let app = TestApp::new(PadzMode::Notes);
        app.seed("Located", "body");

        let result = rendered(path(&app.ctx, vec!["1".into()]).unwrap());

        assert_eq!(result.paths.len(), 1);
        assert!(result.paths[0].contains(".padz"));
        assert!(std::path::Path::new(&result.paths[0]).exists());
    }

    #[test]
    fn uuid_returns_the_pad_uuid_instead_of_printing_it() {
        let app = TestApp::new(PadzMode::Notes);
        app.seed("Identified", "body");

        let expected = {
            let state = get_state(&app.ctx);
            let listed = state
                .with_api(|api| api.view_pads(state.scope, &["1".to_string()], NestingMode::Flat))
                .unwrap();
            listed.listed_pads[0].pad.metadata.id.to_string()
        };

        let result = rendered(uuid(&app.ctx, vec!["1".into()]).unwrap());

        assert_eq!(result.uuids, vec![expected]);
    }

    // --- the mode-independence contract ---------------------------------------

    #[test]
    fn handler_results_serialize_to_one_shape_for_every_output_mode() {
        // There is no mode input to a handler, so there is only one value to serialize.
        // This pins the shape that both the template path and the structured path see.
        let app = TestApp::new(PadzMode::Notes);
        app.seed("Only", "body");

        let result = rendered(
            list(
                &app.ctx,
                vec![],
                None,
                false,
                false,
                false,
                false,
                false,
                false,
                false,
                vec![],
                false,
                false,
            )
            .unwrap(),
        );

        let json = serde_json::to_value(&result).unwrap();
        assert!(json.get("pads").unwrap().is_array());
        assert!(json.get("messages").is_none());
        assert!(json.get("request").unwrap().is_object());

        // Terminal-only derivations must not be anywhere in the handler's value.
        let raw = serde_json::to_string(&json).unwrap();
        for template_only in [
            "title_width",
            "line_width",
            "status_icon",
            "time_ago",
            "left_pin",
        ] {
            assert!(
                !raw.contains(template_only),
                "handler result leaked the template-only field `{template_only}`"
            );
        }
    }

    #[test]
    fn app_state_paths_are_untouched_by_rendering() {
        // AppState carries no OutputMode: nothing in it can branch on presentation.
        let app = TestApp::new(PadzMode::Notes);
        let state = get_state(&app.ctx);
        assert_eq!(state.mode, PadzMode::Notes);
        assert!(state.wants_status(true));
        assert!(!state.wants_status(false));
    }
}
