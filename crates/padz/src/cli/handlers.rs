//! Command handlers for padz CLI.
//!
//! These handlers follow the standout contract:
//! `fn handler_name(matches: &ArgMatches, ctx: &CommandContext) -> HandlerResult<T>`
//!
//! State (API, scope, etc.) is accessed via thread-local storage set by the main run() function.

use clap::ArgMatches;
use padzapp::api::{ConfigAction, PadFilter, PadStatusFilter, PadzApi, TodoStatus};
use padzapp::clipboard::{copy_to_clipboard, format_for_clipboard};
use padzapp::editor::open_in_editor;
use padzapp::model::{extract_title_and_body, parse_pad_content, Scope};
use padzapp::store::fs::FileStore;
use serde_json::Value;
use standout::cli::{CommandContext, HandlerResult, Output};
use standout::OutputMode;
use std::cell::RefCell;
use std::path::Path;

use super::render::{build_modification_result_value, render_pad_list, render_pad_list_deleted};

// Thread-local context for handler state
thread_local! {
    static HANDLER_CONTEXT: RefCell<Option<HandlerContext>> = const { RefCell::new(None) };
}

/// Context shared by all handlers (set by run() before dispatch)
pub struct HandlerContext {
    pub api: PadzApi<FileStore>,
    pub scope: Scope,
    pub import_extensions: Vec<String>,
    pub output_mode: OutputMode,
}

/// Initialize handler context (called by run() before dispatch)
pub fn init_context(ctx: HandlerContext) {
    HANDLER_CONTEXT.with(|c| {
        *c.borrow_mut() = Some(ctx);
    });
}

/// Access handler context (panics if not initialized)
fn with_context<F, R>(f: F) -> R
where
    F: FnOnce(&mut HandlerContext) -> R,
{
    HANDLER_CONTEXT.with(|c| {
        let mut ctx = c.borrow_mut();
        let ctx = ctx.as_mut().expect("Handler context not initialized");
        f(ctx)
    })
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

pub fn create(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let no_editor = matches.get_flag("no_editor");
        let inside: Option<&str> = matches.get_one::<String>("inside").map(|s| s.as_str());
        let title_words: Vec<String> = matches
            .get_many::<String>("title")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let title_arg = if title_words.is_empty() {
            None
        } else {
            Some(title_words.join(" "))
        };

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
            ctx.api
                .create_pad(ctx.scope, final_title, body, inside)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        } else if no_editor {
            // Create with title only (no editor)
            let title = title_arg.unwrap_or_else(|| "Untitled".to_string());
            ctx.api
                .create_pad(ctx.scope, title, String::new(), inside)
                .map_err(|e| anyhow::anyhow!("{}", e))?
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
            ctx.api
                .create_pad(ctx.scope, parsed.title, parsed.content, inside)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        };

        let data = build_modification_result_value(
            "Created",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        Ok(Output::Render(data))
    })
}

pub fn list(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let search: Option<String> = matches.get_one::<String>("search").cloned();
        let deleted = matches.get_flag("deleted");
        let peek = matches.get_flag("peek");
        let planned = matches.get_flag("planned");
        let done = matches.get_flag("done");
        let in_progress = matches.get_flag("in_progress");
        let tags: Vec<String> = matches
            .get_many::<String>("tags")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

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

        let result = ctx
            .api
            .get_pads(ctx.scope, filter)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let output = if deleted {
            render_pad_list_deleted(&result.listed_pads, peek, ctx.output_mode)
        } else {
            render_pad_list(&result.listed_pads, peek, ctx.output_mode)
        };

        // For list, we print directly and return silent (complex rendering)
        print!("{}", output);
        super::render::print_messages(&result.messages, ctx.output_mode);
        Ok(Output::<Value>::Silent)
    })
}

pub fn search(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let term: String = matches
            .get_one::<String>("term")
            .cloned()
            .unwrap_or_default();
        let tags: Vec<String> = matches
            .get_many::<String>("tags")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let filter = PadFilter {
            status: PadStatusFilter::Active,
            search_term: Some(term),
            todo_status: None,
            tags: if tags.is_empty() { None } else { Some(tags) },
        };

        let result = ctx
            .api
            .get_pads(ctx.scope, filter)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let output = render_pad_list(&result.listed_pads, false, ctx.output_mode);
        print!("{}", output);
        super::render::print_messages(&result.messages, ctx.output_mode);
        Ok(Output::<Value>::Silent)
    })
}

// =============================================================================
// Pad operations
// =============================================================================

pub fn view(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        let _peek = matches.get_flag("peek");

        let result = ctx
            .api
            .view_pads(ctx.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

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
    })
}

pub fn edit(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

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
            let result = ctx
                .api
                .update_pads_from_content(ctx.scope, &indexes, &content)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            let data = build_modification_result_value(
                "Updated",
                &result.affected_pads,
                &result.messages,
                ctx.output_mode,
            );
            Ok(Output::Render(data))
        } else {
            // Interactive editor - get pad paths and open each one
            let view_result = ctx
                .api
                .view_pads(ctx.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            for path in &view_result.pad_paths {
                open_in_editor(path).map_err(|e| anyhow::anyhow!("{}", e))?;
            }

            Ok(Output::Render(serde_json::json!({
                "messages": [{"content": format!("Edited {} pad(s)", view_result.pad_paths.len()), "style": "success"}]
            })))
        }
    })
}

pub fn open(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

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
            let result = ctx
                .api
                .update_pads_from_content(ctx.scope, &indexes, &content)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            let data = build_modification_result_value(
                "Updated",
                &result.affected_pads,
                &result.messages,
                ctx.output_mode,
            );
            Ok(Output::Render(data))
        } else {
            // Open in editor and copy to clipboard on exit
            let view_result = ctx
                .api
                .view_pads(ctx.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            // Open each pad's file in editor
            for path in &view_result.pad_paths {
                open_in_editor(path).map_err(|e| anyhow::anyhow!("{}", e))?;
                // Copy to clipboard after editing
                copy_pad_to_clipboard(path);
            }

            Ok(Output::<Value>::Silent)
        }
    })
}

pub fn delete(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        let done_status = matches.get_flag("done_status");

        if done_status {
            // Delete all pads with Done status
            let filter = PadFilter {
                status: PadStatusFilter::Active,
                search_term: None,
                todo_status: Some(TodoStatus::Done),
                tags: None,
            };
            let pads = ctx
                .api
                .get_pads(ctx.scope, filter)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

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

            let result = ctx
                .api
                .delete_pads(ctx.scope, &done_indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            let data = build_modification_result_value(
                "Deleted",
                &result.affected_pads,
                &result.messages,
                ctx.output_mode,
            );
            Ok(Output::Render(data))
        } else {
            let result = ctx
                .api
                .delete_pads(ctx.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            let data = build_modification_result_value(
                "Deleted",
                &result.affected_pads,
                &result.messages,
                ctx.output_mode,
            );
            Ok(Output::Render(data))
        }
    })
}

pub fn restore(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let result = ctx
            .api
            .restore_pads(ctx.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let data = build_modification_result_value(
            "Restored",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        Ok(Output::Render(data))
    })
}

pub fn pin(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let result = ctx
            .api
            .pin_pads(ctx.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let data = build_modification_result_value(
            "Pinned",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        Ok(Output::Render(data))
    })
}

pub fn unpin(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let result = ctx
            .api
            .unpin_pads(ctx.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let data = build_modification_result_value(
            "Unpinned",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        Ok(Output::Render(data))
    })
}

pub fn move_pads(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        let root = matches.get_flag("root");

        let result = if root {
            ctx.api
                .move_pads(ctx.scope, &indexes, None)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        } else {
            // Last index is destination
            if indexes.len() < 2 {
                return Err(anyhow::anyhow!(
                    "Move requires at least 2 arguments (source and destination) or --root flag"
                ));
            }
            let (sources, dest) = indexes.split_at(indexes.len() - 1);
            ctx.api
                .move_pads(ctx.scope, sources, Some(dest[0].as_str()))
                .map_err(|e| anyhow::anyhow!("{}", e))?
        };

        let data = build_modification_result_value(
            "Moved",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        Ok(Output::Render(data))
    })
}

pub fn path(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let result = ctx
            .api
            .pad_paths(ctx.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        for path in &result.pad_paths {
            println!("{}", path.display());
        }
        Ok(Output::<Value>::Silent)
    })
}

pub fn complete(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let result = ctx
            .api
            .complete_pads(ctx.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let data = build_modification_result_value(
            "Completed",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        Ok(Output::Render(data))
    })
}

pub fn reopen(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let result = ctx
            .api
            .reopen_pads(ctx.scope, &indexes)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let data = build_modification_result_value(
            "Reopened",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        Ok(Output::Render(data))
    })
}

pub fn add_tag(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        let tags: Vec<String> = matches
            .get_many::<String>("tags")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let result = ctx
            .api
            .add_tags_to_pads(ctx.scope, &indexes, &tags)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        let data = build_modification_result_value(
            "Tagged",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        Ok(Output::Render(data))
    })
}

pub fn remove_tag(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        let tags: Vec<String> = matches
            .get_many::<String>("tags")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let result = if tags.is_empty() {
            ctx.api
                .clear_tags_from_pads(ctx.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        } else {
            ctx.api
                .remove_tags_from_pads(ctx.scope, &indexes, &tags)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        };

        let data = build_modification_result_value(
            "Untagged",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        Ok(Output::Render(data))
    })
}

// =============================================================================
// Data operations
// =============================================================================

pub fn purge(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();
        let yes = matches.get_flag("yes");
        let recursive = matches.get_flag("recursive");

        let result = ctx
            .api
            .purge_pads(ctx.scope, &indexes, yes, recursive)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    })
}

pub fn export(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let single_file: Option<String> = matches.get_one::<String>("single_file").cloned();
        let indexes: Vec<String> = matches
            .get_many::<String>("indexes")
            .map(|v| v.cloned().collect())
            .unwrap_or_default();

        let result = if let Some(title) = single_file {
            // Single-file export (writes directly to file)
            ctx.api
                .export_pads_single_file(ctx.scope, &indexes, &title)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        } else {
            // Tar.gz export (writes directly to file)
            ctx.api
                .export_pads(ctx.scope, &indexes)
                .map_err(|e| anyhow::anyhow!("{}", e))?
        };

        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    })
}

pub fn import(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let paths: Vec<std::path::PathBuf> = matches
            .get_many::<String>("paths")
            .map(|v| v.map(std::path::PathBuf::from).collect())
            .unwrap_or_default();

        let result = ctx
            .api
            .import_pads(ctx.scope, paths, &ctx.import_extensions)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    })
}

// =============================================================================
// Misc commands
// =============================================================================

pub fn doctor(_matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let result = ctx
            .api
            .doctor(ctx.scope)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    })
}

pub fn config(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let key: Option<String> = matches.get_one::<String>("key").cloned();
        let value: Option<String> = matches.get_one::<String>("value").cloned();

        let action = match (key.clone(), value) {
            (None, _) => ConfigAction::ShowAll,
            (Some(k), None) => ConfigAction::ShowKey(k),
            (Some(k), Some(v)) => ConfigAction::Set(k, v),
        };

        let result = ctx
            .api
            .config(ctx.scope, action)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

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
    })
}

pub fn init(_matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
    with_context(|ctx| {
        let result = ctx
            .api
            .init(ctx.scope)
            .map_err(|e| anyhow::anyhow!("{}", e))?;

        Ok(Output::Render(serde_json::json!({
            "messages": result.messages,
        })))
    })
}

// =============================================================================
// Tags subcommand handlers
// =============================================================================

pub mod tags {
    use super::*;

    pub fn list(_matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
        with_context(|ctx| {
            let result = ctx
                .api
                .list_tags(ctx.scope)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            Ok(Output::Render(serde_json::json!({
                "messages": result.messages,
            })))
        })
    }

    pub fn create(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
        with_context(|ctx| {
            let name: String = matches
                .get_one::<String>("name")
                .cloned()
                .unwrap_or_default();

            let result = ctx
                .api
                .create_tag(ctx.scope, &name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            Ok(Output::Render(serde_json::json!({
                "messages": result.messages,
            })))
        })
    }

    pub fn delete(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
        with_context(|ctx| {
            let name: String = matches
                .get_one::<String>("name")
                .cloned()
                .unwrap_or_default();

            let result = ctx
                .api
                .delete_tag(ctx.scope, &name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            Ok(Output::Render(serde_json::json!({
                "messages": result.messages,
            })))
        })
    }

    pub fn rename(matches: &ArgMatches, _ctx: &CommandContext) -> HandlerResult<Value> {
        with_context(|ctx| {
            let old_name: String = matches
                .get_one::<String>("old_name")
                .cloned()
                .unwrap_or_default();
            let new_name: String = matches
                .get_one::<String>("new_name")
                .cloned()
                .unwrap_or_default();

            let result = ctx
                .api
                .rename_tag(ctx.scope, &old_name, &new_name)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            Ok(Output::Render(serde_json::json!({
                "messages": result.messages,
            })))
        })
    }
}
