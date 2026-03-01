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

use padzapp::api::{PadFilter, PadStatusFilter, PadzApi, TodoStatus};
use padzapp::clipboard::{copy_to_clipboard, format_for_clipboard};
use padzapp::commands::CmdResult;
use padzapp::config::PadzMode;
use padzapp::error::PadzError;
use padzapp::model::{extract_title_and_body, Scope};
use padzapp::store::fs::FileStore;
use serde_json::Value;
use standout::cli::{CommandContext, Output};
use standout::OutputMode;
use standout_macros::handler;
use std::cell::RefCell;
use std::io::IsTerminal;

use super::render::{build_list_result_value, build_modification_result_value};

// =============================================================================
// App State Types (for standout's type-based app_state lookup)
// =============================================================================

/// Wrapper for import extensions list (needed for type-based lookup)
#[derive(Clone)]
pub struct ImportExtensions(pub Vec<String>);

/// Shared application state injected via app_state
/// Contains the API instance wrapped in RefCell for interior mutability
pub struct AppState {
    api: RefCell<PadzApi<FileStore>>,
    pub scope: Scope,
    pub import_extensions: ImportExtensions,
    pub output_mode: OutputMode,
    pub mode: PadzMode,
    /// The local `.padz/` directory (pre-link-resolution), used by link/unlink commands.
    pub local_padz_dir: std::path::PathBuf,
}

impl AppState {
    pub fn new(
        api: PadzApi<FileStore>,
        scope: Scope,
        import_extensions: Vec<String>,
        output_mode: OutputMode,
        mode: PadzMode,
        local_padz_dir: std::path::PathBuf,
    ) -> Self {
        Self {
            api: RefCell::new(api),
            scope,
            import_extensions: ImportExtensions(import_extensions),
            output_mode,
            mode,
            local_padz_dir,
        }
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

/// Scoped API accessor that binds scope and handles error conversion + rendering.
///
/// This wrapper eliminates the repetitive boilerplate pattern:
/// ```ignore
/// let state = get_state(ctx);
/// let result = state.with_api(|api| {
///     api.method(state.scope, &args).map_err(|e| anyhow::anyhow!("{}", e))
/// })?;
/// Ok(Output::Render(build_modification_result_value(...)))
/// ```
///
/// With ScopedApi, handlers become one-liners:
/// ```ignore
/// api(ctx).pin_pads(&indexes)
/// ```
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
            .with_api(|api| f(api, self.state.scope).map_err(|e| anyhow::anyhow!("{}", e)))
    }

    /// Wrap a CmdResult (modification) into rendered output
    fn modification(
        &self,
        action: &str,
        result: CmdResult,
    ) -> Result<Output<Value>, anyhow::Error> {
        Ok(Output::Render(build_modification_result_value(
            action,
            &result.affected_pads,
            &result.messages,
            self.state.output_mode,
            self.state.mode,
        )))
    }

    /// Wrap a CmdResult (messages only) into rendered output
    fn messages(&self, result: CmdResult) -> Result<Output<Value>, anyhow::Error> {
        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    }

    // --- Modification operations ---

    pub fn pin_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.pin_pads(scope, indexes))?;
        self.modification("Pinned", result)
    }

    pub fn unpin_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.unpin_pads(scope, indexes))?;
        self.modification("Unpinned", result)
    }

    pub fn delete_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.delete_pads(scope, indexes))?;
        self.modification("Deleted", result)
    }

    pub fn delete_done_pads(&self) -> Result<Output<Value>, anyhow::Error> {
        let filter = PadFilter {
            status: PadStatusFilter::Active,
            search_term: None,
            todo_status: Some(TodoStatus::Done),
            tags: None,
        };
        let pads = self.call(|api, scope| api.get_pads(scope, filter, &[] as &[String]))?;

        if pads.listed_pads.is_empty() {
            return Ok(Output::Render(serde_json::json!({
                "start_message": "",
                "pads": [],
                "trailing_messages": [{"content": "No done pads to delete.", "style": "info"}]
            })));
        }

        let done_indexes: Vec<String> = pads
            .listed_pads
            .iter()
            .map(|dp| dp.index.to_string())
            .collect();

        self.delete_pads(&done_indexes)
    }

    pub fn restore_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.restore_pads(scope, indexes))?;
        self.modification("Restored", result)
    }

    pub fn archive_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.archive_pads(scope, indexes))?;
        self.modification("Archived", result)
    }

    pub fn unarchive_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.unarchive_pads(scope, indexes))?;
        self.modification("Unarchived", result)
    }

    pub fn complete_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.complete_pads(scope, indexes))?;
        self.modification("Completed", result)
    }

    pub fn reopen_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.reopen_pads(scope, indexes))?;
        self.modification("Reopened", result)
    }

    pub fn move_pads(
        &self,
        indexes: &[String],
        to_root: bool,
    ) -> Result<Output<Value>, anyhow::Error> {
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
        self.modification("Moved", result)
    }

    // --- Message-only operations ---

    pub fn purge_pads(
        &self,
        indexes: &[String],
        yes: bool,
        recursive: bool,
    ) -> Result<Output<Value>, anyhow::Error> {
        let include_done = self.state.mode == PadzMode::Todos;
        let result =
            self.call(|api, scope| api.purge_pads(scope, indexes, recursive, yes, include_done))?;
        self.messages(result)
    }

    pub fn doctor(&self) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.doctor(scope))?;
        self.messages(result)
    }

    pub fn init(&self) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.init(scope))?;
        self.messages(result)
    }

    pub fn init_link(&self, target: &str) -> Result<Output<Value>, anyhow::Error> {
        let target_path = std::path::PathBuf::from(target);
        let local_padz = &self.state.local_padz_dir;
        let result = self.call(|api, _scope| api.init_link(local_padz, &target_path))?;
        self.messages(result)
    }

    pub fn init_unlink(&self) -> Result<Output<Value>, anyhow::Error> {
        let local_padz = &self.state.local_padz_dir;
        let result = self.call(|api, _scope| api.init_unlink(local_padz))?;
        self.messages(result)
    }

    pub fn export_pads(
        &self,
        indexes: &[String],
        single_file: Option<&str>,
    ) -> Result<Output<Value>, anyhow::Error> {
        let result = if let Some(title) = single_file {
            self.call(|api, scope| api.export_pads_single_file(scope, indexes, title))?
        } else {
            self.call(|api, scope| api.export_pads(scope, indexes))?
        };
        self.messages(result)
    }

    pub fn import_pads(
        &self,
        paths: Vec<std::path::PathBuf>,
    ) -> Result<Output<Value>, anyhow::Error> {
        let extensions = &self.state.import_extensions.0;
        let result = self.call(|api, scope| api.import_pads(scope, paths.clone(), extensions))?;
        self.messages(result)
    }

    // --- Tags subcommand operations ---

    pub fn list_tags(&self) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.list_tags(scope))?;
        self.messages(result)
    }

    pub fn delete_tag(&self, name: &str) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.delete_tag(scope, name))?;
        self.messages(result)
    }

    pub fn rename_tag(
        &self,
        old_name: &str,
        new_name: &str,
    ) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.rename_tag(scope, old_name, new_name))?;
        self.messages(result)
    }

    // --- List operations ---

    pub fn list_pads(
        &self,
        filter: PadFilter,
        peek: bool,
        show_deleted_help: bool,
        ids: &[String],
        show_uuid: bool,
    ) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.get_pads(scope, filter, ids))?;
        Ok(Output::Render(build_list_result_value(
            &result.listed_pads,
            peek,
            show_deleted_help,
            &result.messages,
            self.state.output_mode,
            self.state.mode,
            show_uuid,
        )))
    }

    // --- View operations ---

    pub fn view_pads(
        &self,
        indexes: &[String],
        show_uuid: bool,
    ) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.view_pads(scope, indexes))?;

        // Copy content to clipboard
        for dp in &result.listed_pads {
            let clipboard_text = format_for_clipboard(&dp.pad.metadata.title, &dp.pad.content);
            let _ = copy_to_clipboard(&clipboard_text);
        }

        let pads: Vec<serde_json::Value> = result
            .listed_pads
            .iter()
            .map(|dp| {
                let mut v = serde_json::json!({
                    "title": dp.pad.metadata.title,
                    "content": dp.pad.content,
                });
                if show_uuid {
                    v["uuid"] = serde_json::json!(dp.pad.metadata.id.to_string());
                }
                v
            })
            .collect();
        Ok(Output::Render(serde_json::json!({ "pads": pads })))
    }
}

/// Get a scoped API accessor from the command context
fn api(ctx: &CommandContext) -> ScopedApi<'_> {
    ScopedApi {
        state: get_state(ctx),
    }
}

/// Helper to copy pad content to clipboard (from content string)
fn copy_content_to_clipboard(content: &str) {
    if let Some((title, body)) = extract_title_and_body(content) {
        let clipboard_text = format_for_clipboard(&title, &body);
        let _ = copy_to_clipboard(&clipboard_text);
    }
}

/// Try to read content from piped stdin.
/// Returns None if stdin is a terminal (interactive), Some if piped.
fn try_read_stdin() -> Result<Option<String>, anyhow::Error> {
    use std::io::Read;
    if std::io::stdin().is_terminal() {
        return Ok(None);
    }
    let mut content = String::new();
    std::io::stdin()
        .read_to_string(&mut content)
        .map_err(|e| anyhow::anyhow!("Failed to read stdin: {}", e))?;
    Ok(Some(content.trim().to_string()))
}

/// Split a list of args into index selectors and trailing content words.
///
/// Index patterns are: digits, pN, dN, N.N, N-N, pN-pN etc.
/// Once an arg fails to parse as an index, everything from that point is content.
fn split_indexes_and_content(args: &[String]) -> (Vec<String>, Vec<String>) {
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

    (indexes, content)
}

// =============================================================================
// Core commands
// =============================================================================

#[handler]
pub fn create(
    #[ctx] ctx: &CommandContext,
    #[flag] editor: bool,
    #[flag(name = "no_editor")] no_editor: bool,
    #[arg] inside: Option<String>,
    #[arg] title: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    let state = get_state(ctx);
    let title_arg = if title.is_empty() {
        None
    } else {
        Some(title.join(" "))
    };
    let inside = inside.as_deref();

    // --editor forces interactive editor; --no-editor forces skip;
    // default: todos mode with title args skips editor
    let skip_editor =
        !editor && (no_editor || (state.mode == PadzMode::Todos && title_arg.is_some()));

    let result = if skip_editor {
        // Quick-create: use args directly, no editor
        let raw_text = title_arg.unwrap_or_else(|| "Untitled".to_string());
        // Convert literal \n sequences to real newlines
        let expanded = raw_text.replace("\\n", "\n");
        let (title, body) = extract_title_and_body(&expanded)
            .unwrap_or_else(|| ("Untitled".to_string(), String::new()));
        let result = state.with_api(|api| {
            api.create_pad(state.scope, title.clone(), body.clone(), inside)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;
        // Copy to clipboard
        let clipboard_text = format_for_clipboard(&title, &body);
        let _ = copy_to_clipboard(&clipboard_text);
        result
    } else if let Some(raw) = try_read_stdin()? {
        // Piped content from stdin
        if raw.is_empty() {
            return Ok(Output::Render(serde_json::json!({
                "start_message": "",
                "pads": [],
                "trailing_messages": [{"content": "Aborted: empty content", "style": "warning"}]
            })));
        }
        let parsed = padzapp::editor::EditorContent::from_buffer(&raw);
        // Determine title: title_arg override > parsed title > "Untitled"
        let final_title = match (&title_arg, parsed.title.is_empty()) {
            (Some(t), _) => t.clone(),  // CLI title always wins
            (_, false) => parsed.title, // parsed has title, no CLI override
            (None, true) => "Untitled".to_string(),
        };
        let result = state.with_api(|api| {
            api.create_pad(state.scope, final_title, parsed.content, inside)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;
        // Copy to clipboard
        copy_content_to_clipboard(&raw);
        result
    } else {
        // Interactive: create pad first, then open real file in editor
        let initial_title = title_arg.clone().unwrap_or_else(|| "Untitled".to_string());
        let create_result = state.with_api(|api| {
            api.create_pad(state.scope, initial_title, String::new(), inside)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;
        let pad_path = create_result.pad_paths[0].clone();
        let pad_id = create_result.affected_pads[0].pad.metadata.id;

        // Open editor on the real pad file in .padz/
        if let Err(e) = padzapp::editor::open_in_editor(&pad_path) {
            // Editor failed - clean up the pad
            let _ = state.with_api(|api| api.remove_pad(state.scope, pad_id));
            return Err(anyhow::anyhow!("{}", e));
        }

        // Refresh pad from disk (re-reads content, updates title)
        match state.with_api(|api| {
            api.refresh_pad(state.scope, &pad_id)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })? {
            Some(pad) => {
                // Copy to clipboard
                let clipboard_text = format_for_clipboard(&pad.metadata.title, &pad.content);
                let _ = copy_to_clipboard(&clipboard_text);
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
                return Ok(Output::Render(serde_json::json!({
                    "start_message": "",
                    "pads": [],
                    "trailing_messages": [{"content": "Aborted: empty content", "style": "warning"}]
                })));
            }
        }
    };

    let data = build_modification_result_value(
        "Created",
        &result.affected_pads,
        &result.messages,
        state.output_mode,
        state.mode,
    );
    Ok(Output::Render(data))
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
    #[flag] peek: bool,
    #[flag] planned: bool,
    #[flag] done: bool,
    #[flag(name = "in_progress")] in_progress: bool,
    #[arg] tags: Vec<String>,
    #[flag] uuid: bool,
) -> Result<Output<Value>, anyhow::Error> {
    let todo_status = if planned {
        Some(TodoStatus::Planned)
    } else if done {
        Some(TodoStatus::Done)
    } else if in_progress {
        Some(TodoStatus::InProgress)
    } else {
        None
    };

    let filter = PadFilter {
        status: if deleted {
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

    api(ctx).list_pads(filter, peek, deleted || archived, &ids, uuid)
}

#[handler]
pub fn peek(
    #[ctx] ctx: &CommandContext,
    #[arg] ids: Vec<String>,
    #[arg] tags: Vec<String>,
    #[flag] uuid: bool,
) -> Result<Output<Value>, anyhow::Error> {
    let filter = PadFilter {
        status: PadStatusFilter::Active,
        search_term: None,
        todo_status: None,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };
    api(ctx).list_pads(filter, true, false, &ids, uuid)
}

#[handler]
pub fn search(
    #[ctx] ctx: &CommandContext,
    #[arg] term: String,
    #[arg] tags: Vec<String>,
    #[flag] uuid: bool,
) -> Result<Output<Value>, anyhow::Error> {
    let filter = PadFilter {
        status: PadStatusFilter::Active,
        search_term: Some(term),
        todo_status: None,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };

    api(ctx).list_pads(filter, false, false, &[], uuid)
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
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).view_pads(&indexes, uuid)
}

#[handler]
pub fn edit(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    let state = get_state(ctx);

    // Split args into index selectors and trailing content words.
    // Index patterns: digits, pN, dN, N.N, N-N, pN-pN, etc.
    let (index_args, content_words) = split_indexes_and_content(&indexes);

    if index_args.is_empty() {
        return Err(anyhow::anyhow!("No pad index provided"));
    }

    // In todos mode, trailing content words become a quick-edit (skip editor)
    let inline_content = if !content_words.is_empty() {
        let raw_text = content_words.join(" ");
        let expanded = raw_text.replace("\\n", "\n");
        Some(expanded)
    } else {
        None
    };

    let skip_editor = state.mode == PadzMode::Todos && inline_content.is_some();

    if skip_editor {
        let raw = inline_content.unwrap();
        let result = state.with_api(|api| {
            api.update_pads_from_content(state.scope, &index_args, &raw)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        let clipboard_text = raw.clone();
        let _ = copy_to_clipboard(&clipboard_text);

        let data = build_modification_result_value(
            "Updated",
            &result.affected_pads,
            &result.messages,
            state.output_mode,
            state.mode,
        );
        return Ok(Output::Render(data));
    }

    // Check for piped stdin
    if let Some(raw) = try_read_stdin()? {
        if raw.is_empty() {
            return Err(anyhow::anyhow!("Aborted: empty content"));
        }
        let result = state.with_api(|api| {
            api.update_pads_from_content(state.scope, &index_args, &raw)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        copy_content_to_clipboard(&raw);

        let data = build_modification_result_value(
            "Updated",
            &result.affected_pads,
            &result.messages,
            state.output_mode,
            state.mode,
        );
        return Ok(Output::Render(data));
    }

    // Interactive editor: open real pad file
    let view_result = state.with_api(|api| {
        api.view_pads(state.scope, &index_args)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    let pad = view_result
        .listed_pads
        .first()
        .ok_or_else(|| anyhow::anyhow!("No pad found"))?;
    let pad_id = pad.pad.metadata.id;
    let display_index = pad.index.clone();

    let pad_path = state.with_api(|api| {
        api.get_path_by_id(state.scope, pad_id)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    // Open editor on the real pad file in .padz/
    padzapp::editor::open_in_editor(&pad_path)?;

    // Refresh pad from disk (re-reads content, updates title)
    match state.with_api(|api| {
        api.refresh_pad(state.scope, &pad_id)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })? {
        Some(pad) => {
            copy_content_to_clipboard(&pad.content);

            let display_pad = padzapp::index::DisplayPad {
                pad,
                index: display_index,
                matches: None,
                children: Vec::new(),
            };
            let result = CmdResult {
                affected_pads: vec![display_pad],
                ..Default::default()
            };
            let data = build_modification_result_value(
                "Updated",
                &result.affected_pads,
                &result.messages,
                state.output_mode,
                state.mode,
            );
            Ok(Output::Render(data))
        }
        None => {
            // User emptied the file
            Ok(Output::<Value>::Silent)
        }
    }
}

#[handler]
pub fn delete(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag(name = "done_status")] done_status: bool,
) -> Result<Output<Value>, anyhow::Error> {
    if done_status {
        api(ctx).delete_done_pads()
    } else {
        api(ctx).delete_pads(&indexes)
    }
}

#[handler]
pub fn restore(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).restore_pads(&indexes)
}

#[handler]
pub fn archive(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).archive_pads(&indexes)
}

#[handler]
pub fn unarchive(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).unarchive_pads(&indexes)
}

/// Pin pads to the top of the list.
#[handler]
pub fn pin(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).pin_pads(&indexes)
}

#[handler]
pub fn unpin(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).unpin_pads(&indexes)
}

#[handler]
pub fn move_pads(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag] root: bool,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).move_pads(&indexes, root)
}

/// Returns pad file paths - outputs directly and returns Silent.
#[handler]
pub fn path(#[ctx] ctx: &CommandContext, #[arg] indexes: Vec<String>) -> Result<(), anyhow::Error> {
    let state = get_state(ctx);
    let result = state.with_api(|api| {
        api.pad_paths(state.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    for path in &result.pad_paths {
        println!("{}", path.display());
    }
    Ok(())
}

/// Returns pad UUIDs - outputs directly and returns Silent.
#[handler]
pub fn uuid(#[ctx] ctx: &CommandContext, #[arg] indexes: Vec<String>) -> Result<(), anyhow::Error> {
    let state = get_state(ctx);
    let result = state.with_api(|api| {
        api.pad_uuids(state.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    for msg in &result.messages {
        println!("{}", msg.content);
    }
    Ok(())
}

#[handler]
pub fn complete(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).complete_pads(&indexes)
}

#[handler]
pub fn reopen(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
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
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).purge_pads(&indexes, yes, recursive)
}

#[handler]
pub fn export(
    #[ctx] ctx: &CommandContext,
    #[arg(name = "single_file")] single_file: Option<String>,
    #[arg] indexes: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).export_pads(&indexes, single_file.as_deref())
}

#[handler]
pub fn import(
    #[ctx] ctx: &CommandContext,
    #[arg] paths: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    let paths: Vec<std::path::PathBuf> = paths.into_iter().map(std::path::PathBuf::from).collect();
    api(ctx).import_pads(paths)
}

// =============================================================================
// Misc commands
// =============================================================================

#[handler]
pub fn doctor(#[ctx] ctx: &CommandContext) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).doctor()
}

#[handler]
pub fn init(
    #[ctx] ctx: &CommandContext,
    #[arg] link: Option<String>,
    #[flag] unlink: bool,
) -> Result<Output<Value>, anyhow::Error> {
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
    ) -> Result<Output<Value>, anyhow::Error> {
        let (indexes, tags) = split_indexes_and_tags(&args)?;
        let state = get_state(ctx);
        let result = state.with_api(|api| {
            api.add_tags_to_pads(state.scope, &indexes, &tags)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;
        Ok(Output::Render(build_modification_result_value(
            "Tagged",
            &result.affected_pads,
            &result.messages,
            state.output_mode,
            state.mode,
        )))
    }

    #[handler]
    pub fn remove(
        #[ctx] ctx: &CommandContext,
        #[arg] args: Vec<String>,
    ) -> Result<Output<Value>, anyhow::Error> {
        let (indexes, tags) = split_indexes_and_tags(&args)?;
        let state = get_state(ctx);
        let result = state.with_api(|api| {
            api.remove_tags_from_pads(state.scope, &indexes, &tags)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;
        Ok(Output::Render(build_modification_result_value(
            "Untagged",
            &result.affected_pads,
            &result.messages,
            state.output_mode,
            state.mode,
        )))
    }

    #[handler]
    pub fn rename(
        #[ctx] ctx: &CommandContext,
        #[arg(name = "old_name")] old_name: String,
        #[arg(name = "new_name")] new_name: String,
    ) -> Result<Output<Value>, anyhow::Error> {
        api(ctx).rename_tag(&old_name, &new_name)
    }

    #[handler]
    pub fn delete(
        #[ctx] ctx: &CommandContext,
        #[arg] name: String,
    ) -> Result<Output<Value>, anyhow::Error> {
        api(ctx).delete_tag(&name)
    }

    #[handler]
    pub fn list(
        #[ctx] ctx: &CommandContext,
        #[arg] ids: Vec<String>,
    ) -> Result<Output<Value>, anyhow::Error> {
        if ids.is_empty() {
            api(ctx).list_tags()
        } else {
            let state = get_state(ctx);
            let result = state.with_api(|api| {
                api.list_pad_tags(state.scope, &ids)
                    .map_err(|e| anyhow::anyhow!("{}", e))
            })?;
            Ok(Output::Render(serde_json::json!({
                "messages": result.messages,
            })))
        }
    }
}
