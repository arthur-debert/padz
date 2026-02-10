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

use clap::ArgMatches;
use padzapp::api::{ConfigAction, PadFilter, PadStatusFilter, PadzApi, TodoStatus};
use padzapp::clipboard::{copy_to_clipboard, format_for_clipboard};
use padzapp::commands::CmdResult;
use padzapp::error::PadzError;
use padzapp::model::{extract_title_and_body, Scope};
use padzapp::store::fs::FileStore;
use serde_json::Value;
use standout::cli::{CommandContext, Output};
use standout::OutputMode;
use standout_input::{EditorSource, InputChain, StdinSource};
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
        let pads = self.call(|api, scope| api.get_pads(scope, filter))?;

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

    pub fn complete_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.complete_pads(scope, indexes))?;
        self.modification("Completed", result)
    }

    pub fn reopen_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.reopen_pads(scope, indexes))?;
        self.modification("Reopened", result)
    }

    pub fn add_tags(
        &self,
        indexes: &[String],
        tags: &[String],
    ) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.add_tags_to_pads(scope, indexes, tags))?;
        self.modification("Tagged", result)
    }

    pub fn remove_tags(
        &self,
        indexes: &[String],
        tags: &[String],
    ) -> Result<Output<Value>, anyhow::Error> {
        let result = if tags.is_empty() {
            self.call(|api, scope| api.clear_tags_from_pads(scope, indexes))?
        } else {
            self.call(|api, scope| api.remove_tags_from_pads(scope, indexes, tags))?
        };
        self.modification("Untagged", result)
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
        let result = self.call(|api, scope| api.purge_pads(scope, indexes, yes, recursive))?;
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

    pub fn create_tag(&self, name: &str) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.create_tag(scope, name))?;
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
    ) -> Result<Output<Value>, anyhow::Error> {
        let result = self.call(|api, scope| api.get_pads(scope, filter))?;
        Ok(Output::Render(build_list_result_value(
            &result.listed_pads,
            peek,
            show_deleted_help,
            &result.messages,
            self.state.output_mode,
        )))
    }

    // --- View operations ---

    pub fn view_pads(&self, indexes: &[String]) -> Result<Output<Value>, anyhow::Error> {
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
                serde_json::json!({
                    "title": dp.pad.metadata.title,
                    "content": dp.pad.content,
                })
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

/// Build the initial editor content template for a new pad
fn make_editor_template(title: Option<&str>) -> String {
    let title = title.unwrap_or_default();
    padzapp::editor::EditorContent::new(title.to_string(), String::new()).to_buffer()
}

/// Collect content from stdin or editor using InputChain
fn collect_content(
    matches: &ArgMatches,
    initial_content: &str,
) -> Result<Option<String>, anyhow::Error> {
    let result = InputChain::<String>::new()
        .try_source(StdinSource::new())
        .try_source(EditorSource::new().initial_content(initial_content))
        .resolve(matches);

    match result {
        Ok(content) => Ok(Some(content)),
        Err(standout_input::InputError::NoInput) => Ok(None),
        Err(e) => Err(anyhow::anyhow!("{}", e)),
    }
}

// =============================================================================
// Core commands
// =============================================================================

#[handler]
pub fn create(
    #[ctx] ctx: &CommandContext,
    #[flag(name = "no_editor")] no_editor: bool,
    #[arg] inside: Option<String>,
    #[arg] title: Vec<String>,
    #[matches] matches: &ArgMatches,
) -> Result<Output<Value>, anyhow::Error> {
    let state = get_state(ctx);
    let title_arg = if title.is_empty() {
        None
    } else {
        Some(title.join(" "))
    };
    let inside = inside.as_deref();

    let result = if no_editor {
        // --no-editor: create with title only, no content collection
        let title = title_arg.unwrap_or_else(|| "Untitled".to_string());
        let result = state.with_api(|api| {
            api.create_pad(state.scope, title.clone(), String::new(), inside)
                .map_err(|e| anyhow::anyhow!("{}", e))
        })?;
        // Copy to clipboard
        let clipboard_text = format_for_clipboard(&title, "");
        let _ = copy_to_clipboard(&clipboard_text);
        result
    } else {
        // Collect content via InputChain: stdin → editor
        let template = make_editor_template(title_arg.as_deref());
        let content = collect_content(matches, &template)?;

        match content {
            Some(raw) if !raw.trim().is_empty() => {
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
            }
            _ => {
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
    #[flag(name = "in_progress")] in_progress: bool,
    #[arg] tags: Vec<String>,
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
        } else {
            PadStatusFilter::Active
        },
        search_term: search,
        todo_status,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };

    api(ctx).list_pads(filter, peek, deleted)
}

#[handler]
pub fn search(
    #[ctx] ctx: &CommandContext,
    #[arg] term: String,
    #[arg] tags: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    let filter = PadFilter {
        status: PadStatusFilter::Active,
        search_term: Some(term),
        todo_status: None,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };

    api(ctx).list_pads(filter, false, false)
}

// =============================================================================
// Pad operations
// =============================================================================

#[handler]
pub fn view(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[flag(name = "peek")] _peek: bool, // Reserved for future use
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).view_pads(&indexes)
}

#[handler]
pub fn edit(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[matches] matches: &ArgMatches,
) -> Result<Output<Value>, anyhow::Error> {
    let state = get_state(ctx);

    // Get existing pad content to use as initial editor content
    let view_result = state.with_api(|api| {
        api.view_pads(state.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))
    })?;

    // For now, only support single pad edit (first pad)
    let pad = view_result
        .listed_pads
        .first()
        .ok_or_else(|| anyhow::anyhow!("No pad found"))?;

    // Build editor template with existing content
    let existing_content = padzapp::editor::EditorContent::new(
        pad.pad.metadata.title.clone(),
        pad.pad.content.clone(),
    )
    .to_buffer();

    // Collect content via InputChain: stdin → editor
    let content = collect_content(matches, &existing_content)?;

    match content {
        Some(raw) if !raw.trim().is_empty() => {
            // Update the pad using the raw content (API parses title/body)
            let result = state.with_api(|api| {
                api.update_pads_from_content(state.scope, &indexes, &raw)
                    .map_err(|e| anyhow::anyhow!("{}", e))
            })?;

            // Copy to clipboard
            copy_content_to_clipboard(&raw);

            let data = build_modification_result_value(
                "Updated",
                &result.affected_pads,
                &result.messages,
                state.output_mode,
            );
            Ok(Output::Render(data))
        }
        Some(_) | None if !std::io::stdin().is_terminal() => {
            Err(anyhow::anyhow!("Aborted: empty content"))
        }
        _ => Ok(Output::<Value>::Silent),
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

#[handler]
pub fn add_tag(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[arg] tags: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).add_tags(&indexes, &tags)
}

#[handler]
pub fn remove_tag(
    #[ctx] ctx: &CommandContext,
    #[arg] indexes: Vec<String>,
    #[arg] tags: Vec<String>,
) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).remove_tags(&indexes, &tags)
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
pub fn config(
    #[ctx] ctx: &CommandContext,
    #[arg] key: Option<String>,
    #[arg] value: Option<String>,
) -> Result<Output<Value>, anyhow::Error> {
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
pub fn init(#[ctx] ctx: &CommandContext) -> Result<Output<Value>, anyhow::Error> {
    api(ctx).init()
}

// =============================================================================
// Tags subcommand handlers
// =============================================================================

pub mod tags {
    use super::*;

    #[handler]
    pub fn list(#[ctx] ctx: &CommandContext) -> Result<Output<Value>, anyhow::Error> {
        api(ctx).list_tags()
    }

    #[handler]
    pub fn create(
        #[ctx] ctx: &CommandContext,
        #[arg] name: String,
    ) -> Result<Output<Value>, anyhow::Error> {
        api(ctx).create_tag(&name)
    }

    #[handler]
    pub fn delete(
        #[ctx] ctx: &CommandContext,
        #[arg] name: String,
    ) -> Result<Output<Value>, anyhow::Error> {
        api(ctx).delete_tag(&name)
    }

    #[handler]
    pub fn rename(
        #[ctx] ctx: &CommandContext,
        #[arg(name = "old_name")] old_name: String,
        #[arg(name = "new_name")] new_name: String,
    ) -> Result<Output<Value>, anyhow::Error> {
        api(ctx).rename_tag(&old_name, &new_name)
    }
}
