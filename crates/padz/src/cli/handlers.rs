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

use padzapp::api::{ConfigAction, PadFilter, PadStatusFilter, PadzApi, TodoStatus};
use padzapp::clipboard::{copy_to_clipboard, format_for_clipboard};
use padzapp::commands::CmdResult;
use padzapp::editor::open_in_editor;
use padzapp::error::PadzError;
use padzapp::model::{extract_title_and_body, parse_pad_content, Scope};
use padzapp::store::fs::FileStore;
use serde_json::Value;
use standout::cli::{CommandContext, HandlerResult, Output};
use standout::OutputMode;
use standout_macros::handler;
use std::cell::RefCell;
use std::path::Path;

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
}

impl AppState {
    pub fn new(
        api: PadzApi<FileStore>,
        scope: Scope,
        import_extensions: Vec<String>,
        output_mode: OutputMode,
    ) -> Self {
        Self {
            api: RefCell::new(api),
            scope,
            import_extensions: ImportExtensions(import_extensions),
            output_mode,
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
    fn modification(&self, action: &str, result: CmdResult) -> HandlerResult<Value> {
        Ok(Output::Render(build_modification_result_value(
            action,
            &result.affected_pads,
            &result.messages,
            self.state.output_mode,
        )))
    }

    // --- Modification operations ---

    pub fn pin_pads(&self, indexes: &[String]) -> HandlerResult<Value> {
        let result = self.call(|api, scope| api.pin_pads(scope, indexes))?;
        self.modification("Pinned", result)
    }
}

/// Get a scoped API accessor from the command context
fn api(ctx: &CommandContext) -> ScopedApi<'_> {
    ScopedApi {
        state: get_state(ctx),
    }
}

/// Helper to copy pad content to clipboard
fn copy_pad_to_clipboard(path: &Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Some((title, body)) = extract_title_and_body(&content) {
            let clipboard_text = format_for_clipboard(&title, &body);
            let _ = copy_to_clipboard(&clipboard_text);
        }
    }
}

// =============================================================================
// Core commands
// =============================================================================

#[handler]
pub fn create(
    #[ctx] ctx: &CommandContext,
    #[flag] no_editor: bool,
    #[arg] inside: Option<String>,
    #[arg] title: Vec<String>,
) -> HandlerResult<Value> {
    let state = get_state(ctx);

    let title_arg = if title.is_empty() {
        None
    } else {
        Some(title.join(" "))
    };
    let inside = inside.as_deref();

    // Check for piped input
    use std::io::{IsTerminal, Read};
    let piped_content = if !std::io::stdin().is_terminal() {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .ok()
            .filter(|_| !buffer.trim().is_empty())
            .map(|_| buffer)
    } else {
        None
    };

    let result = if let Some(piped) = piped_content {
        // Create from piped content - parse to get title and body
        let (title, body) = parse_pad_content(&piped)
            .ok_or_else(|| anyhow::anyhow!("Invalid content: could not extract title"))?;
        // If title_arg provided, use it as title override
        let final_title = title_arg.unwrap_or(title);
        state.with_api(|api| {
            api.create_pad(state.scope, final_title, body, inside)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?
    } else if no_editor {
        // Create with title only (no editor)
        let title = title_arg.unwrap_or_else(|| "Untitled".to_string());
        state.with_api(|api| {
            api.create_pad(state.scope, title, String::new(), inside)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?
    } else {
        // Interactive editor creation - use temp file
        let initial_title = title_arg.unwrap_or_default();
        let editor_content = padzapp::editor::EditorContent::new(initial_title, String::new());

        // Create temp file
        let temp_dir = std::env::temp_dir();
        let temp_path = temp_dir.join(format!("padz-{}.txt", std::process::id()));
        std::fs::write(&temp_path, editor_content.to_buffer())
            .map_err(|e| anyhow::anyhow!("Failed to create temp file: {}", e))?;

        // Open in editor
        open_in_editor(&temp_path).map_err(|e| anyhow::anyhow!("Editor error: {}", e))?;

        // Read result
        let edited = std::fs::read_to_string(&temp_path)
            .map_err(|e| anyhow::anyhow!("Failed to read temp file: {}", e))?;

        // Clean up temp file
        let _ = std::fs::remove_file(&temp_path);

        if edited.trim().is_empty() {
            return Ok(Output::Render(serde_json::json!({
                "start_message": "",
                "pads": [],
                "trailing_messages": [{"content": "Aborted: empty content", "style": "warning"}]
            })));
        }

        let parsed = padzapp::editor::EditorContent::from_buffer(&edited);
        state.with_api(|api| {
            api.create_pad(state.scope, parsed.title, parsed.content, inside)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?
    };

    let data = build_modification_result_value(
        "Created",
        &result.affected_pads,
        &result.messages,
        state.output_mode,
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
    #[arg] search: Option<String>,
    #[flag] deleted: bool,
    #[flag] peek: bool,
    #[flag] planned: bool,
    #[flag] done: bool,
    #[flag(name = "in-progress")] in_progress: bool,
    #[arg] tags: Vec<String>,
) -> HandlerResult<Value> {
    let state = get_state(ctx);

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
        } else {
            PadStatusFilter::Active
        },
        search_term: search,
        todo_status,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };

    let result = state.with_api(|api| {
        api.get_pads(state.scope, filter)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    Ok(Output::Render(build_list_result_value(
        &result.listed_pads,
        peek,
        deleted,
        &result.messages,
        state.output_mode,
    )))
}

#[handler]
pub fn search(
    #[ctx] ctx: &CommandContext,
    #[arg] term: String,
    #[arg] tags: Vec<String>,
) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let filter = PadFilter {
        status: PadStatusFilter::Active,
        search_term: Some(term),
        todo_status: None,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };

    let result = state.with_api(|api| {
        api.get_pads(state.scope, filter)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    Ok(Output::Render(build_list_result_value(
        &result.listed_pads,
        false, // peek
        false, // show_deleted_help
        &result.messages,
        state.output_mode,
    )))
}

// =============================================================================
// Pad operations
// =============================================================================

#[handler]
pub fn view(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag] _peek: bool, // Reserved for future use
) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = state.with_api(|api| {
        api.view_pads(state.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    // Build data for template rendering
    let pads: Vec<serde_json::Value> = result
        .listed_pads
        .iter()
        .map(|dp| {
            serde_json::json!({
                "title": dp.pad.metadata.title,
                "content": dp.pad.content,
            })
        })
        .collect();

    Ok(Output::Render(serde_json::json!({ "pads": pads })))
}

#[handler]
pub fn edit(#[ctx] ctx: &CommandContext, #[arg] indexes: Vec<String>) -> HandlerResult<Value> {
    let state = get_state(ctx);

    // Check for piped input
    use std::io::{IsTerminal, Read};
    let piped_content = if !std::io::stdin().is_terminal() {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .ok()
            .filter(|_| !buffer.trim().is_empty())
            .map(|_| buffer)
    } else {
        None
    };

    if let Some(content) = piped_content {
        // Update from piped content
        let result = state.with_api(|api| {
            api.update_pads_from_content(state.scope, &indexes, &content)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        let data = build_modification_result_value(
            "Updated",
            &result.affected_pads,
            &result.messages,
            state.output_mode,
        );
        Ok(Output::Render(data))
    } else {
        // Interactive editor - get pad paths and open each one
        let view_result = state.with_api(|api| {
            api.view_pads(state.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        for path in &view_result.pad_paths {
            open_in_editor(path).map_err(|e| anyhow::anyhow!("{}", e))?;
        }

        Ok(Output::Render(serde_json::json!({
            "messages": [{"content": format!("Edited {} pad(s)", view_result.pad_paths.len()), "style": "success"}]
        })))
    }
}

#[handler]
pub fn open(#[ctx] ctx: &CommandContext, #[arg] indexes: Vec<String>) -> HandlerResult<Value> {
    let state = get_state(ctx);

    // Check for piped input
    use std::io::{IsTerminal, Read};
    let piped_content = if !std::io::stdin().is_terminal() {
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .ok()
            .filter(|_| !buffer.trim().is_empty())
            .map(|_| buffer)
    } else {
        None
    };

    if let Some(content) = piped_content {
        // Update from piped content (same as edit)
        let result = state.with_api(|api| {
            api.update_pads_from_content(state.scope, &indexes, &content)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        let data = build_modification_result_value(
            "Updated",
            &result.affected_pads,
            &result.messages,
            state.output_mode,
        );
        Ok(Output::Render(data))
    } else {
        // Open in editor and copy to clipboard on exit
        let view_result = state.with_api(|api| {
            api.view_pads(state.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        // Open each pad's file in editor
        for path in &view_result.pad_paths {
            open_in_editor(path).map_err(|e| anyhow::anyhow!("{}", e))?;
            // Copy to clipboard after editing
            copy_pad_to_clipboard(path);
        }

        Ok(Output::<Value>::Silent)
    }
}

#[handler]
pub fn delete(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag] done_status: bool,
) -> HandlerResult<Value> {
    let state = get_state(ctx);

    if done_status {
        // Delete all pads with Done status
        let filter = PadFilter {
            status: PadStatusFilter::Active,
            search_term: None,
            todo_status: Some(TodoStatus::Done),
            tags: None,
        };
        let pads = state.with_api(|api| {
            api.get_pads(state.scope, filter)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

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

        let result = state.with_api(|api| {
            api.delete_pads(state.scope, &done_indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        let data = build_modification_result_value(
            "Deleted",
            &result.affected_pads,
            &result.messages,
            state.output_mode,
        );
        Ok(Output::Render(data))
    } else {
        let result = state.with_api(|api| {
            api.delete_pads(state.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        let data = build_modification_result_value(
            "Deleted",
            &result.affected_pads,
            &result.messages,
            state.output_mode,
        );
        Ok(Output::Render(data))
    }
}

#[handler]
pub fn restore(#[ctx] ctx: &CommandContext, #[arg] indexes: Vec<String>) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = state.with_api(|api| {
        api.restore_pads(state.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    Ok(Output::Render(build_modification_result_value(
        "Restored",
        &result.affected_pads,
        &result.messages,
        state.output_mode,
    )))
}

/// Pin pads to the top of the list.
#[handler]
pub fn pin(#[ctx] ctx: &CommandContext, #[arg] indexes: Vec<String>) -> HandlerResult<Value> {
    api(ctx).pin_pads(&indexes)
}

#[handler]
pub fn unpin(#[ctx] ctx: &CommandContext, #[arg] indexes: Vec<String>) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = state.with_api(|api| {
        api.unpin_pads(state.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    Ok(Output::Render(build_modification_result_value(
        "Unpinned",
        &result.affected_pads,
        &result.messages,
        state.output_mode,
    )))
}

#[handler]
pub fn move_pads(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag] root: bool,
) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = if root {
        state.with_api(|api| {
            api.move_pads(state.scope, &indexes, None)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?
    } else {
        // Last index is destination
        if indexes.len() < 2 {
            return Err(anyhow::anyhow!(
                "Move requires at least 2 arguments (source and destination) or --root flag"
            ));
        }
        let (sources, dest) = indexes.split_at(indexes.len() - 1);
        state.with_api(|api| {
            api.move_pads(state.scope, sources, Some(dest[0].as_str()))
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?
    };

    Ok(Output::Render(build_modification_result_value(
        "Moved",
        &result.affected_pads,
        &result.messages,
        state.output_mode,
    )))
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

#[handler]
pub fn complete(#[ctx] ctx: &CommandContext, #[arg] indexes: Vec<String>) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = state.with_api(|api| {
        api.complete_pads(state.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    Ok(Output::Render(build_modification_result_value(
        "Completed",
        &result.affected_pads,
        &result.messages,
        state.output_mode,
    )))
}

#[handler]
pub fn reopen(#[ctx] ctx: &CommandContext, #[arg] indexes: Vec<String>) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = state.with_api(|api| {
        api.reopen_pads(state.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    Ok(Output::Render(build_modification_result_value(
        "Reopened",
        &result.affected_pads,
        &result.messages,
        state.output_mode,
    )))
}

#[handler]
pub fn add_tag(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[arg] tags: Vec<String>,
) -> HandlerResult<Value> {
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
    )))
}

#[handler]
pub fn remove_tag(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[arg] tags: Vec<String>,
) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = if tags.is_empty() {
        state.with_api(|api| {
            api.clear_tags_from_pads(state.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?
    } else {
        state.with_api(|api| {
            api.remove_tags_from_pads(state.scope, &indexes, &tags)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?
    };

    Ok(Output::Render(build_modification_result_value(
        "Untagged",
        &result.affected_pads,
        &result.messages,
        state.output_mode,
    )))
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
) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = state.with_api(|api| {
        api.purge_pads(state.scope, &indexes, yes, recursive)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    Ok(Output::Render(serde_json::json!({
        "messages": result.messages,
    })))
}

#[handler]
pub fn export(
    #[ctx] ctx: &CommandContext,
    #[arg(name = "single-file")] single_file: Option<String>,
    #[arg] indexes: Vec<String>,
) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = if let Some(title) = single_file {
        // Single-file export (writes directly to file)
        state.with_api(|api| {
            api.export_pads_single_file(state.scope, &indexes, &title)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?
    } else {
        // Tar.gz export (writes directly to file)
        state.with_api(|api| {
            api.export_pads(state.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?
    };

    Ok(Output::Render(serde_json::json!({
        "messages": result.messages,
    })))
}

#[handler]
pub fn import(#[ctx] ctx: &CommandContext, #[arg] paths: Vec<String>) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let paths: Vec<std::path::PathBuf> = paths.into_iter().map(std::path::PathBuf::from).collect();

    let result = state.with_api(|api| {
        api.import_pads(state.scope, paths, &state.import_extensions.0)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    Ok(Output::Render(serde_json::json!({
        "messages": result.messages,
    })))
}

// =============================================================================
// Misc commands
// =============================================================================

#[handler]
pub fn doctor(#[ctx] ctx: &CommandContext) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result = state.with_api(|api| {
        api.doctor(state.scope)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    Ok(Output::Render(serde_json::json!({
        "messages": result.messages,
    })))
}

#[handler]
pub fn config(
    #[ctx] ctx: &CommandContext,
    #[arg] key: Option<String>,
    #[arg] value: Option<String>,
) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let action = match (key.clone(), value) {
        (None, _) => ConfigAction::ShowAll,
        (Some(k), None) => ConfigAction::ShowKey(k),
        (Some(k), Some(v)) => ConfigAction::Set(k, v),
    };

    let result = state.with_api(|api| {
        api.config(state.scope, action)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    let mut lines = Vec::new();
    if let Some(config) = &result.config {
        if key.is_none() {
            for (k, v) in config.list_all() {
                lines.push(format!("{} = {}", k, v));
            }
        }
    }

    Ok(Output::Render(serde_json::json!({
        "lines": lines,
        "empty_message": "No configuration values.",
        "messages": result.messages,
    })))
}

#[handler]
pub fn init(#[ctx] ctx: &CommandContext) -> HandlerResult<Value> {
    let state = get_state(ctx);
    let result =
        state.with_api(|api| api.init(state.scope).map_err(|e| anyhow::anyhow!("{}", e)))?;

    Ok(Output::Render(serde_json::json!({
        "messages": result.messages,
    })))
}

// =============================================================================
// Tags subcommand handlers
// =============================================================================

pub mod tags {
    use super::*;

    #[handler]
    pub fn list(#[ctx] ctx: &CommandContext) -> HandlerResult<Value> {
        let state = get_state(ctx);
        let result = state.with_api(|api| {
            api.list_tags(state.scope)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    }

    #[handler]
    pub fn create(#[ctx] ctx: &CommandContext, #[arg] name: String) -> HandlerResult<Value> {
        let state = get_state(ctx);
        let result = state.with_api(|api| {
            api.create_tag(state.scope, &name)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    }

    #[handler]
    pub fn delete(#[ctx] ctx: &CommandContext, #[arg] name: String) -> HandlerResult<Value> {
        let state = get_state(ctx);
        let result = state.with_api(|api| {
            api.delete_tag(state.scope, &name)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    }

    #[handler]
    pub fn rename(
        #[ctx] ctx: &CommandContext,
        #[arg(name = "old-name")] old_name: String,
        #[arg(name = "new-name")] new_name: String,
    ) -> HandlerResult<Value> {
        let state = get_state(ctx);
        let result = state.with_api(|api| {
            api.rename_tag(state.scope, &old_name, &new_name)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;

        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    }
}
