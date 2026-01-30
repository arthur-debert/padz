//! # CLI Layer
//!
//! This module is **one possible UI client** for padz—it is not the application itself.
//!
//! The CLI layer is the **only** place in the codebase that:
//! - Knows about terminal I/O (stdout, stderr)
//! - Uses `std::process::exit`
//! - Handles argument parsing
//! - Formats output for human consumption
//!
//! ## Responsibilities
//!
//! 1. **Argument Parsing**: Convert shell arguments into typed commands via clap
//! 2. **Context Setup**: Initialize `AppContext` with API, scope, and configuration
//! 3. **API Dispatch**: Call the appropriate `PadzApi` method
//! 4. **Output Formatting**: Convert `CmdResult` into terminal output (colors, tables, etc.)
//! 5. **Error Handling**: Convert errors to user-friendly messages and exit codes
//!
//! ## Testing Strategy
//!
//! CLI tests verify two directions:
//!
//! **Input Testing**: Given shell argument strings, verify:
//! - Arguments parse correctly
//! - Correct API method is called
//! - Arguments are passed correctly to API
//!
//! **Output Testing**: Given a `CmdResult`, verify:
//! - Correct text is written to stdout
//! - Colors and formatting are applied correctly
//! - Error messages go to stderr
//!
//! CLI tests should **not** test business logic—that's the command layer's job.
//!
//! ## Structure
//!
//! - `run()`: Main dispatch logic (called by `main.rs`)
//! - `init_context()`: Builds `AppContext` with API and configuration
//! - `handle_*()`: Per-command handlers that call API and format output
//! - `print_*()`: Output formatting functions

use super::render::{
    build_modification_result_value, print_messages, render_modification_result, render_pad_list,
    render_pad_list_deleted,
};
use super::setup::{
    build_command, parse_cli, Cli, Commands, CompletionShell, CoreCommands, MiscCommands,
};
use padzapp::api::{ConfigAction, PadFilter, PadStatusFilter, PadzApi, TodoStatus};
use padzapp::clipboard::{copy_to_clipboard, format_for_clipboard, get_from_clipboard};
use padzapp::editor::open_in_editor;
use padzapp::error::Result;
use padzapp::init::initialize;
use padzapp::model::Scope;
use padzapp::model::{extract_title_and_body, parse_pad_content};
use padzapp::store::fs::FileStore;
use standout::cli::handler::RunResult;
use standout::cli::{LocalApp, Output};
use standout::{embed_styles, embed_templates, OutputMode};
use std::cell::RefCell;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Helper to read a pad file and copy its content to the system clipboard.
/// Silently ignores errors (clipboard operations are best-effort).
fn copy_pad_to_clipboard(path: &Path) {
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Some((title, body)) = extract_title_and_body(&content) {
            let clipboard_text = format_for_clipboard(&title, &body);
            let _ = copy_to_clipboard(&clipboard_text);
        }
    }
}

struct AppContext {
    api: PadzApi<FileStore>,
    scope: Scope,
    import_extensions: Vec<String>,
    output_mode: OutputMode,
}

pub fn run() -> Result<()> {
    // parse_cli() uses standout's App which handles
    // help display (including topics) and errors automatically.
    // It also extracts the output mode from the --output flag.
    let (cli, output_mode) = parse_cli();

    // Handle completions before context init (they don't need API)
    if let Some(Commands::Misc(MiscCommands::Completions { shell })) = &cli.command {
        return handle_completions(*shell);
    }

    let ctx = init_context(&cli, output_mode)?;

    // Wrap API in Rc<RefCell> for sharing between LocalApp handlers and fallback code
    let api = Rc::new(RefCell::new(ctx.api));
    let scope = ctx.scope;
    let output_mode_val = ctx.output_mode;
    let import_extensions = ctx.import_extensions;

    // Shared inline template for modification results
    const MODIFICATION_TEMPLATE: &str = r#"{%- if start_message -%}
[info]{{ start_message }}[/info]
{% endif -%}
{%- for pad in pads -%}
{{ pad.left_pin | col(2) }}{{ pad.status_icon | col(2) }}{{ pad.index | col(4) }}{{ pad.title | col(pad.title_width) }}{{ pad.right_pin | col(2) }}{{ pad.time_ago | col(14, align="right") }}
{% endfor -%}
{%- for msg in trailing_messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#;

    // Scope LocalApp so it's dropped before fallback unwrap
    // This ensures the Rc clones captured by closures are released
    {
        let api_for_complete = api.clone();
        let api_for_reopen = api.clone();
        let api_for_pin = api.clone();
        let api_for_unpin = api.clone();
        let api_for_delete = api.clone();
        let api_for_restore = api.clone();
        let api_for_move = api.clone();
        let api_for_add_tag = api.clone();
        let api_for_remove_tag = api.clone();
        let api_for_view = api.clone();
        let api_for_edit = api.clone();
        let api_for_open = api.clone();
        let api_for_path = api.clone();
        let api_for_purge = api.clone();
        let api_for_export = api.clone();
        let api_for_import = api.clone();
        let api_for_tags_list = api.clone();
        let api_for_tags_create = api.clone();
        let api_for_tags_delete = api.clone();
        let api_for_tags_rename = api.clone();
        let api_for_doctor = api.clone();
        let api_for_config = api.clone();
        let api_for_init = api.clone();
        let import_extensions_clone = import_extensions.clone();

        let local_app = LocalApp::builder()
            .templates(embed_templates!("src/cli/templates"))
            .styles(embed_styles!("src/styles"))
            .default_theme("default")
            .command(
                "complete",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let mut api_ref = api_for_complete.borrow_mut();
                    let result = api_ref
                        .complete_pads(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    let data = build_modification_result_value(
                        "Completed",
                        &result.affected_pads,
                        &result.messages,
                        OutputMode::Term,
                    );

                    Ok(Output::Render(data))
                },
                MODIFICATION_TEMPLATE,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            .command(
                "reopen",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let mut api_ref = api_for_reopen.borrow_mut();
                    let result = api_ref
                        .reopen_pads(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    let data = build_modification_result_value(
                        "Reopened",
                        &result.affected_pads,
                        &result.messages,
                        OutputMode::Term,
                    );

                    Ok(Output::Render(data))
                },
                MODIFICATION_TEMPLATE,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            .command(
                "pin",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let mut api_ref = api_for_pin.borrow_mut();
                    let result = api_ref
                        .pin_pads(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    let data = build_modification_result_value(
                        "Pinned",
                        &result.affected_pads,
                        &result.messages,
                        OutputMode::Term,
                    );

                    Ok(Output::Render(data))
                },
                MODIFICATION_TEMPLATE,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            .command(
                "unpin",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let mut api_ref = api_for_unpin.borrow_mut();
                    let result = api_ref
                        .unpin_pads(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    let data = build_modification_result_value(
                        "Unpinned",
                        &result.affected_pads,
                        &result.messages,
                        OutputMode::Term,
                    );

                    Ok(Output::Render(data))
                },
                MODIFICATION_TEMPLATE,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            .command(
                "delete",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();
                    let done_status = matches.get_flag("done_status");

                    let mut api_ref = api_for_delete.borrow_mut();

                    if done_status {
                        // Delete all pads with Done status
                        let filter = PadFilter {
                            status: PadStatusFilter::Active,
                            search_term: None,
                            todo_status: Some(TodoStatus::Done),
                            tags: None,
                        };
                        let pads = api_ref
                            .get_pads(scope, filter)
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

                        let result = api_ref
                            .delete_pads(scope, &done_indexes)
                            .map_err(|e| anyhow::anyhow!("{}", e))?;

                        let data = build_modification_result_value(
                            "Deleted",
                            &result.affected_pads,
                            &result.messages,
                            OutputMode::Term,
                        );

                        Ok(Output::Render(data))
                    } else {
                        let result = api_ref
                            .delete_pads(scope, &indexes)
                            .map_err(|e| anyhow::anyhow!("{}", e))?;

                        let data = build_modification_result_value(
                            "Deleted",
                            &result.affected_pads,
                            &result.messages,
                            OutputMode::Term,
                        );

                        Ok(Output::Render(data))
                    }
                },
                MODIFICATION_TEMPLATE,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            .command(
                "restore",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let mut api_ref = api_for_restore.borrow_mut();
                    let result = api_ref
                        .restore_pads(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    let data = build_modification_result_value(
                        "Restored",
                        &result.affected_pads,
                        &result.messages,
                        OutputMode::Term,
                    );

                    Ok(Output::Render(data))
                },
                MODIFICATION_TEMPLATE,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            .command(
                "move",
                move |matches, _cmd_ctx| {
                    let mut indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();
                    let root = matches.get_flag("root");

                    let destination = if root {
                        None
                    } else {
                        if indexes.len() < 2 {
                            return Err(anyhow::anyhow!(
                                "Missing destination. Use `padz move <SOURCE>... <DEST>` or `padz move <SOURCE>... --root`"
                            ));
                        }
                        Some(indexes.pop().unwrap())
                    };

                    let mut api_ref = api_for_move.borrow_mut();
                    let result = api_ref
                        .move_pads(scope, &indexes, destination.as_deref())
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    let data = build_modification_result_value(
                        "Moved",
                        &result.affected_pads,
                        &result.messages,
                        OutputMode::Term,
                    );

                    Ok(Output::Render(data))
                },
                MODIFICATION_TEMPLATE,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            .command(
                "add-tag",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();
                    let tags: Vec<String> = matches
                        .get_many::<String>("tags")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let mut api_ref = api_for_add_tag.borrow_mut();
                    let result = api_ref
                        .add_tags_to_pads(scope, &indexes, &tags)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    let data = build_modification_result_value(
                        "Tagged",
                        &result.affected_pads,
                        &result.messages,
                        OutputMode::Term,
                    );

                    Ok(Output::Render(data))
                },
                MODIFICATION_TEMPLATE,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            .command(
                "remove-tag",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();
                    let tags: Vec<String> = matches
                        .get_many::<String>("tags")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let mut api_ref = api_for_remove_tag.borrow_mut();
                    let result = if tags.is_empty() {
                        api_ref
                            .clear_tags_from_pads(scope, &indexes)
                            .map_err(|e| anyhow::anyhow!("{}", e))?
                    } else {
                        api_ref
                            .remove_tags_from_pads(scope, &indexes, &tags)
                            .map_err(|e| anyhow::anyhow!("{}", e))?
                    };

                    let data = build_modification_result_value(
                        "Untagged",
                        &result.affected_pads,
                        &result.messages,
                        OutputMode::Term,
                    );

                    Ok(Output::Render(data))
                },
                MODIFICATION_TEMPLATE,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- path command ---
            .command(
                "path",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let api_ref = api_for_path.borrow();
                    let result = api_ref
                        .pad_paths(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    let lines: Vec<String> = result
                        .pad_paths
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect();

                    Ok(Output::Render(serde_json::json!({
                        "lines": lines,
                        "empty_message": "No pad paths found.",
                        "messages": result.messages,
                    })))
                },
                // text_list template
                r#"{%- if lines -%}
{%- for line in lines -%}
{{ line }}
{% endfor -%}
{%- else -%}
{{ empty_message }}
{% endif -%}
{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- edit command (no output, just opens editor) ---
            .command(
                "edit",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let api_ref = api_for_edit.borrow();
                    let result = api_ref
                        .view_pads(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    // Side effects: open in editor and copy to clipboard
                    for path in &result.pad_paths {
                        open_in_editor(path).map_err(|e| anyhow::anyhow!("{}", e))?;
                        copy_pad_to_clipboard(path);
                    }

                    Ok(Output::<serde_json::Value>::Silent)
                },
                "", // No template needed for Silent output
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- open command (piped input updates pad, otherwise like edit) ---
            .command(
                "open",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    // Check for piped input
                    if !std::io::stdin().is_terminal() {
                        let mut buffer = String::new();
                        if std::io::stdin().read_to_string(&mut buffer).is_ok() {
                            if buffer.trim().is_empty() {
                                return Err(anyhow::anyhow!("Piped content is empty"));
                            }
                            // Update the pad(s) with the piped content
                            let mut api_ref = api_for_open.borrow_mut();
                            let result = api_ref
                                .update_pads_from_content(scope, &indexes, &buffer)
                                .map_err(|e| anyhow::anyhow!("{}", e))?;

                            let data = build_modification_result_value(
                                "Updated",
                                &result.affected_pads,
                                &result.messages,
                                OutputMode::Term,
                            );

                            return Ok(Output::Render(data));
                        }
                    }

                    // No piped input - behave like edit (open the file in editor)
                    let api_ref = api_for_open.borrow();
                    let result = api_ref
                        .view_pads(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    for path in &result.pad_paths {
                        open_in_editor(path).map_err(|e| anyhow::anyhow!("{}", e))?;
                        copy_pad_to_clipboard(path);
                    }

                    Ok(Output::<serde_json::Value>::Silent)
                },
                MODIFICATION_TEMPLATE, // Used when piped input updates pad
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- purge command ---
            .command(
                "purge",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();
                    let yes = matches.get_flag("yes");
                    let recursive = matches.get_flag("recursive");

                    let mut api_ref = api_for_purge.borrow_mut();
                    let result = api_ref
                        .purge_pads(scope, &indexes, recursive, yes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    Ok(Output::Render(serde_json::json!({
                        "messages": result.messages,
                    })))
                },
                // messages-only template
                r#"{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- export command ---
            .command(
                "export",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();
                    let single_file: Option<String> = matches
                        .get_one::<String>("single_file")
                        .cloned();

                    let api_ref = api_for_export.borrow();
                    let result = if let Some(title) = single_file {
                        api_ref
                            .export_pads_single_file(scope, &indexes, &title)
                            .map_err(|e| anyhow::anyhow!("{}", e))?
                    } else {
                        api_ref
                            .export_pads(scope, &indexes)
                            .map_err(|e| anyhow::anyhow!("{}", e))?
                    };

                    Ok(Output::Render(serde_json::json!({
                        "messages": result.messages,
                    })))
                },
                r#"{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- import command ---
            .command(
                "import",
                move |matches, _cmd_ctx| {
                    let paths: Vec<String> = matches
                        .get_many::<String>("paths")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();
                    let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();

                    let mut api_ref = api_for_import.borrow_mut();
                    let result = api_ref
                        .import_pads(scope, paths, &import_extensions_clone)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    Ok(Output::Render(serde_json::json!({
                        "messages": result.messages,
                    })))
                },
                r#"{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- tags list command ---
            .command(
                "tags.list",
                move |_matches, _cmd_ctx| {
                    let api_ref = api_for_tags_list.borrow();
                    let result = api_ref
                        .list_tags(scope)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    Ok(Output::Render(serde_json::json!({
                        "messages": result.messages,
                    })))
                },
                r#"{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- tags create command ---
            .command(
                "tags.create",
                move |matches, _cmd_ctx| {
                    let name: String = matches
                        .get_one::<String>("name")
                        .cloned()
                        .unwrap_or_default();

                    let mut api_ref = api_for_tags_create.borrow_mut();
                    let result = api_ref
                        .create_tag(scope, &name)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    Ok(Output::Render(serde_json::json!({
                        "messages": result.messages,
                    })))
                },
                r#"{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- tags delete command ---
            .command(
                "tags.delete",
                move |matches, _cmd_ctx| {
                    let name: String = matches
                        .get_one::<String>("name")
                        .cloned()
                        .unwrap_or_default();

                    let mut api_ref = api_for_tags_delete.borrow_mut();
                    let result = api_ref
                        .delete_tag(scope, &name)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    Ok(Output::Render(serde_json::json!({
                        "messages": result.messages,
                    })))
                },
                r#"{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- tags rename command ---
            .command(
                "tags.rename",
                move |matches, _cmd_ctx| {
                    let old_name: String = matches
                        .get_one::<String>("old_name")
                        .cloned()
                        .unwrap_or_default();
                    let new_name: String = matches
                        .get_one::<String>("new_name")
                        .cloned()
                        .unwrap_or_default();

                    let mut api_ref = api_for_tags_rename.borrow_mut();
                    let result = api_ref
                        .rename_tag(scope, &old_name, &new_name)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    Ok(Output::Render(serde_json::json!({
                        "messages": result.messages,
                    })))
                },
                r#"{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- doctor command ---
            .command(
                "doctor",
                move |_matches, _cmd_ctx| {
                    let mut api_ref = api_for_doctor.borrow_mut();
                    let result = api_ref
                        .doctor(scope)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    Ok(Output::Render(serde_json::json!({
                        "messages": result.messages,
                    })))
                },
                r#"{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- config command ---
            .command(
                "config",
                move |matches, _cmd_ctx| {
                    let key: Option<String> = matches.get_one::<String>("key").cloned();
                    let value: Option<String> = matches.get_one::<String>("value").cloned();

                    let action = match (key.clone(), value) {
                        (None, _) => ConfigAction::ShowAll,
                        (Some(k), None) => ConfigAction::ShowKey(k),
                        (Some(k), Some(v)) => ConfigAction::Set(k, v),
                    };

                    let api_ref = api_for_config.borrow_mut();
                    let result = api_ref
                        .config(scope, action)
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
                },
                r#"{%- if lines -%}
{%- for line in lines -%}
{{ line }}
{% endfor -%}
{%- else -%}
{{ empty_message }}
{% endif -%}
{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- init command ---
            .command(
                "init",
                move |_matches, _cmd_ctx| {
                    let api_ref = api_for_init.borrow();
                    let result = api_ref
                        .init(scope)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    Ok(Output::Render(serde_json::json!({
                        "messages": result.messages,
                    })))
                },
                r#"{%- for msg in messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            // --- view command ---
            .command(
                "view",
                move |matches, _cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();
                    let peek = matches.get_flag("peek");

                    let api_ref = api_for_view.borrow();
                    let result = api_ref
                        .view_pads(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    // Build template data
                    use padzapp::index::DisplayIndex;
                    let pads: Vec<serde_json::Value> = result
                        .listed_pads
                        .iter()
                        .map(|dp| {
                            let is_pinned = matches!(dp.index, DisplayIndex::Pinned(_));
                            let is_deleted = matches!(dp.index, DisplayIndex::Deleted(_));
                            serde_json::json!({
                                "index": format!("{}", dp.index),
                                "title": dp.pad.metadata.title,
                                "content": dp.pad.content,
                                "is_pinned": is_pinned,
                                "is_deleted": is_deleted,
                            })
                        })
                        .collect();

                    // Copy viewed pads to clipboard (side effect)
                    if !result.listed_pads.is_empty() {
                        let clipboard_text: String = result
                            .listed_pads
                            .iter()
                            .map(|dp| dp.pad.content.clone())
                            .collect::<Vec<_>>()
                            .join("\n\n---\n\n");
                        let _ = copy_to_clipboard(&clipboard_text);
                    }

                    // If peek mode, return list-style data instead
                    if peek {
                        // For peek, we use a simpler format - just show titles
                        return Ok(Output::Render(serde_json::json!({
                            "pads": pads,
                            "peek": true,
                        })));
                    }

                    Ok(Output::Render(serde_json::json!({
                        "pads": pads,
                    })))
                },
                "full_pad.jinja", // Use registered template
            )
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?
            .build()
            .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?;

        // Try LocalApp dispatch first
        let dispatch_result = local_app.dispatch_from(build_command(), std::env::args());

        match dispatch_result {
            RunResult::Handled(output) => {
                if !output.is_empty() {
                    println!("{}", output);
                }
                return Ok(());
            }
            RunResult::Binary(bytes, filename) => {
                std::fs::write(&filename, &bytes)?;
                eprintln!("Wrote {} bytes to {}", bytes.len(), filename);
                return Ok(());
            }
            RunResult::Silent => {
                return Ok(());
            }
            RunResult::NoMatch(_) => {
                // Fall through to fallback dispatch
            }
        }
    } // LocalApp and api_for_complete dropped here

    // Fallback dispatch for non-migrated commands
    // Now safe to unwrap the Rc since LocalApp released its clone
    let mut ctx = AppContext {
        api: Rc::try_unwrap(api)
            .map_err(|_| {
                padzapp::error::PadzError::Api(
                    "BUG: API Rc should have single owner after LocalApp drop".to_string(),
                )
            })?
            .into_inner(),
        scope,
        import_extensions,
        output_mode: output_mode_val,
    };

    // Fallback dispatch handles only commands not yet migrated to LocalApp:
    // - CoreCommands (list, search, create) - complex rendering/input handling
    // - None case (naked invocation defaults to list or create)
    // All other commands are handled by LocalApp above.
    match cli.command {
        Some(Commands::Core(cmd)) => match cmd {
            CoreCommands::Create {
                title,
                no_editor,
                inside,
            } => {
                let title = if title.is_empty() {
                    None
                } else {
                    Some(title.join(" "))
                };
                handle_create(&mut ctx, title, no_editor, inside)
            }
            CoreCommands::List {
                search,
                deleted,
                peek,
                planned,
                done,
                in_progress,
                tags,
            } => handle_list(
                &mut ctx,
                search,
                deleted,
                peek,
                planned,
                done,
                in_progress,
                tags,
            ),
            CoreCommands::Search { term, tags } => handle_search(&mut ctx, term, tags),
        },
        None => {
            // Naked invocation: check for piped input
            // `cat file.txt | padz` expands to `padz create` with piped content
            // `padz` (no pipe) expands to `padz list`
            if !std::io::stdin().is_terminal() {
                handle_create(&mut ctx, None, false, None)
            } else {
                handle_list(&mut ctx, None, false, false, false, false, false, vec![])
            }
        }
        // All other commands are handled by LocalApp - this branch should never be reached
        _ => unreachable!("Command should have been handled by LocalApp"),
    }
}

fn init_context(cli: &Cli, output_mode: OutputMode) -> Result<AppContext> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let data_override = cli.data.as_ref().map(PathBuf::from);
    let ctx = initialize(&cwd, cli.global, data_override);

    Ok(AppContext {
        api: ctx.api,
        scope: ctx.scope,
        import_extensions: ctx.config.import_extensions.clone(),
        output_mode,
    })
}

fn handle_create(
    ctx: &mut AppContext,
    title: Option<String>,
    no_editor: bool,
    inside: Option<String>,
) -> Result<()> {
    // === Pre-dispatch: Resolve input from stdin/clipboard ===
    let (final_title, initial_content, should_open_editor) = resolve_create_input(title, no_editor);

    // === Dispatch: Call API ===
    let title_to_use = final_title.unwrap_or_else(|| "Untitled".to_string());
    let parent = inside.as_deref();
    let result = ctx
        .api
        .create_pad(ctx.scope, title_to_use, initial_content, parent)?;

    // === Output: Render unified modification result ===
    let output = render_modification_result(
        "Created",
        &result.affected_pads,
        &result.messages,
        ctx.output_mode,
    );
    print!("{}", output);

    // === Post-dispatch: Editor and clipboard side effects ===
    if should_open_editor && !result.pad_paths.is_empty() {
        let path = &result.pad_paths[0];
        open_in_editor(path)?;
        copy_pad_to_clipboard(path);
    }

    Ok(())
}

/// Pre-dispatch logic for create: resolve title and content from stdin/clipboard.
/// Returns (title, content, should_open_editor).
fn resolve_create_input(title: Option<String>, no_editor: bool) -> (Option<String>, String, bool) {
    let mut final_title = title;
    let mut initial_content = String::new();
    let mut should_open_editor = !no_editor;

    // 1. Check for piped input (stdin)
    if !std::io::stdin().is_terminal() {
        let mut buffer = String::new();
        if std::io::stdin().read_to_string(&mut buffer).is_ok() && !buffer.trim().is_empty() {
            if final_title.is_none() {
                if let Some((parsed_title, _)) = parse_pad_content(&buffer) {
                    final_title = Some(parsed_title);
                }
            }
            initial_content = buffer;
            should_open_editor = false; // Piped input skips editor
        }
    }

    // 2. If still no content/title, check clipboard
    if final_title.is_none() && initial_content.is_empty() {
        if let Ok(clipboard_content) = get_from_clipboard() {
            if !clipboard_content.trim().is_empty() {
                if let Some((parsed_title, _)) = parse_pad_content(&clipboard_content) {
                    final_title = Some(parsed_title);
                }
                initial_content = clipboard_content;
            }
        }
    }

    (final_title, initial_content, should_open_editor)
}

#[allow(clippy::too_many_arguments)]
fn handle_list(
    ctx: &mut AppContext,
    search: Option<String>,
    deleted: bool,
    peek: bool,
    planned: bool,
    done: bool,
    in_progress: bool,
    tags: Vec<String>,
) -> Result<()> {
    // Determine todo status filter
    let todo_status = if planned {
        Some(TodoStatus::Planned)
    } else if done {
        Some(TodoStatus::Done)
    } else if in_progress {
        Some(TodoStatus::InProgress)
    } else {
        None // No filter = show all
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

    let result = ctx.api.get_pads(ctx.scope, filter)?;

    // Use standout-based rendering
    let output = if deleted {
        render_pad_list_deleted(&result.listed_pads, peek, ctx.output_mode)
    } else {
        render_pad_list(&result.listed_pads, peek, ctx.output_mode)
    };
    print!("{}", output);

    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_search(ctx: &mut AppContext, term: String, tags: Vec<String>) -> Result<()> {
    let filter = PadFilter {
        status: PadStatusFilter::Active,
        search_term: Some(term),
        todo_status: None,
        tags: if tags.is_empty() { None } else { Some(tags) },
    };
    let result = ctx.api.get_pads(ctx.scope, filter)?;
    let output = render_pad_list(&result.listed_pads, false, ctx.output_mode);
    print!("{}", output);
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_completions(shell: CompletionShell) -> Result<()> {
    // Output the shell setup script generated by clap_complete
    // Users should add to their shell rc: eval "$(padz completions bash)"
    use super::setup::build_command;
    use clap_complete::env::{CompleteEnv, EnvCompleter};

    let shell_name = match shell {
        CompletionShell::Bash => "bash",
        CompletionShell::Zsh => "zsh",
    };

    // Generate the shell completion script by simulating the COMPLETE env var
    // clap_complete outputs the registration script when COMPLETE is set
    let completer = CompleteEnv::with_factory(build_command);
    let mut buf = Vec::new();

    match shell {
        CompletionShell::Bash => {
            clap_complete::env::Bash
                .write_registration("COMPLETE", "padz", "padz", "padz", &mut buf)
                .expect("Failed to generate bash completions");
        }
        CompletionShell::Zsh => {
            clap_complete::env::Zsh
                .write_registration("COMPLETE", "padz", "padz", "padz", &mut buf)
                .expect("Failed to generate zsh completions");
        }
    }

    println!("# {} completion for padz", shell_name);
    println!(
        "# Add to your shell rc file: eval \"$(padz completions {})\"",
        shell_name
    );
    println!();
    print!("{}", String::from_utf8_lossy(&buf));

    // Suppress unused variable warning
    let _ = completer;

    Ok(())
}
