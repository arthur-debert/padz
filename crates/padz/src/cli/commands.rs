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
    build_modification_result_value, print_messages, render_full_pads, render_modification_result,
    render_pad_list, render_pad_list_deleted, render_text_list,
};
use super::setup::{
    build_command, parse_cli, Cli, Commands, CompletionShell, CoreCommands, DataCommands,
    MiscCommands, PadCommands, TagsCommands,
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
use standout::OutputMode;
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

    // Scope LocalApp so it's dropped before fallback unwrap
    // This ensures the Rc clones captured by closures are released
    {
        let api_for_complete = api.clone();

        let mut local_app = LocalApp::builder()
            .command(
                "complete",
                move |matches, cmd_ctx| {
                    let indexes: Vec<String> = matches
                        .get_many::<String>("indexes")
                        .map(|v| v.cloned().collect())
                        .unwrap_or_default();

                    let mut api_ref = api_for_complete.borrow_mut();
                    let result = api_ref
                        .complete_pads(scope, &indexes)
                        .map_err(|e| anyhow::anyhow!("{}", e))?;

                    // Build template data appropriate for the output mode
                    let data = build_modification_result_value(
                        "Completed",
                        &result.affected_pads,
                        &result.messages,
                        cmd_ctx.output_mode,
                    );

                    Ok(Output::Render(data))
                },
                // Inline template - can't use {% include %} with LocalApp's inline templates
                // This is a simplified version for the POC
                r#"{%- if start_message -%}
[info]{{ start_message }}[/info]
{% endif -%}
{%- for pad in pads -%}
{{ pad.left_pin | col(2) }}{{ pad.status_icon | col(2) }}{{ pad.index | col(4) }}{{ pad.title | col(pad.title_width) }}{{ pad.right_pin | col(2) }}{{ pad.time_ago | col(14, align="right") }}
{% endfor -%}
{%- for msg in trailing_messages -%}
{{ msg.content | style_as(msg.style) }}
{% endfor -%}"#,
            )
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

    match cli.command {
        Some(Commands::Core(cmd)) => match cmd {
            CoreCommands::Create {
                title,
                no_editor,
                inside,
            } => {
                // Join all title words with spaces
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
        Some(Commands::Pad(cmd)) => match cmd {
            PadCommands::View { indexes, peek } => handle_view(&mut ctx, indexes, peek),
            PadCommands::Edit { indexes } => handle_edit(&mut ctx, indexes),
            PadCommands::Open { indexes } => handle_open(&mut ctx, indexes),
            PadCommands::Delete {
                indexes,
                done_status,
            } => handle_delete(&mut ctx, indexes, done_status),
            PadCommands::Restore { indexes } => handle_restore(&mut ctx, indexes),
            PadCommands::Pin { indexes } => handle_pin(&mut ctx, indexes),
            PadCommands::Unpin { indexes } => handle_unpin(&mut ctx, indexes),
            PadCommands::Path { indexes } => handle_paths(&mut ctx, indexes),
            PadCommands::Complete { indexes } => handle_complete(&mut ctx, indexes),
            PadCommands::Reopen { indexes } => handle_reopen(&mut ctx, indexes),
            PadCommands::Move { indexes, root } => handle_move(&mut ctx, indexes, root),
            PadCommands::AddTag { indexes, tags } => handle_add_tag(&mut ctx, indexes, tags),
            PadCommands::RemoveTag { indexes, tags } => handle_remove_tag(&mut ctx, indexes, tags),
        },
        Some(Commands::Data(cmd)) => match cmd {
            DataCommands::Purge {
                indexes,
                yes,
                recursive,
            } => handle_purge(&mut ctx, indexes, yes, recursive),
            DataCommands::Export {
                single_file,
                indexes,
            } => handle_export(&mut ctx, indexes, single_file),
            DataCommands::Import { paths } => handle_import(&mut ctx, paths),
        },
        Some(Commands::Tags(cmd)) => match cmd {
            TagsCommands::List => handle_tags_list(&mut ctx),
            TagsCommands::Create { name } => handle_tags_create(&mut ctx, name),
            TagsCommands::Delete { name } => handle_tags_delete(&mut ctx, name),
            TagsCommands::Rename { old_name, new_name } => {
                handle_tags_rename(&mut ctx, old_name, new_name)
            }
        },
        Some(Commands::Misc(cmd)) => match cmd {
            MiscCommands::Doctor => handle_doctor(&mut ctx),
            MiscCommands::Config { key, value } => handle_config(&mut ctx, key, value),
            MiscCommands::Init => handle_init(&ctx),
            MiscCommands::Completions { shell } => handle_completions(shell),
        },
        None => {
            // Naked invocation: check for piped input
            // `cat file.txt | padz` expands to `padz create` with piped content
            // `padz` (no pipe) expands to `padz list`
            if !std::io::stdin().is_terminal() {
                // Piped input detected - expand to create
                handle_create(&mut ctx, None, false, None)
            } else {
                // No piped input - default to list
                handle_list(&mut ctx, None, false, false, false, false, false, vec![])
            }
        }
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

fn handle_view(ctx: &mut AppContext, indexes: Vec<String>, peek: bool) -> Result<()> {
    let result = ctx.api.view_pads(ctx.scope, &indexes)?;
    let output = if peek {
        // Reuse list rendering for peek view
        render_pad_list(&result.listed_pads, true, ctx.output_mode)
    } else {
        render_full_pads(&result.listed_pads, ctx.output_mode)
    };
    print!("{}", output);
    print_messages(&result.messages, ctx.output_mode);

    // Copy viewed pads to clipboard
    // Note: dp.pad.content already includes the title as the first line
    if !result.listed_pads.is_empty() {
        let clipboard_text: String = result
            .listed_pads
            .iter()
            .map(|dp| dp.pad.content.clone())
            .collect::<Vec<_>>()
            .join("\n\n---\n\n");
        let _ = copy_to_clipboard(&clipboard_text);
    }

    Ok(())
}

fn handle_edit(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    // === Dispatch: Call API (view returns paths) ===
    let result = ctx.api.view_pads(ctx.scope, &indexes)?;

    // === Post-dispatch: Editor and clipboard side effects ===
    for path in &result.pad_paths {
        open_in_editor(path)?;
        copy_pad_to_clipboard(path);
    }

    Ok(())
}

fn handle_open(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    // Check for piped input - if present, update pad content instead of opening editor
    if !std::io::stdin().is_terminal() {
        let mut buffer = String::new();
        if std::io::stdin().read_to_string(&mut buffer).is_ok() {
            if buffer.trim().is_empty() {
                // Piped input detected but empty - this is an error
                return Err(padzapp::error::PadzError::Api(
                    "Piped content is empty".to_string(),
                ));
            }
            // Update the pad(s) with the piped content
            let result = ctx
                .api
                .update_pads_from_content(ctx.scope, &indexes, &buffer)?;

            let output = render_modification_result(
                "Updated",
                &result.affected_pads,
                &result.messages,
                ctx.output_mode,
            );
            print!("{}", output);
            return Ok(());
        }
    }

    // No piped input - behave like edit (open the file in editor)
    handle_edit(ctx, indexes)
}

fn handle_delete(ctx: &mut AppContext, indexes: Vec<String>, done_status: bool) -> Result<()> {
    if done_status {
        // Delete all pads with Done status
        let filter = PadFilter {
            status: PadStatusFilter::Active,
            search_term: None,
            todo_status: Some(TodoStatus::Done),
            tags: None,
        };
        let pads = ctx.api.get_pads(ctx.scope, filter)?;

        if pads.listed_pads.is_empty() {
            println!("No done pads to delete.");
            return Ok(());
        }

        // Collect indexes of done pads
        let done_indexes: Vec<String> = pads
            .listed_pads
            .iter()
            .map(|dp| dp.index.to_string())
            .collect();

        let result = ctx.api.delete_pads(ctx.scope, &done_indexes)?;
        let output = render_modification_result(
            "Deleted",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        print!("{}", output);
    } else {
        let result = ctx.api.delete_pads(ctx.scope, &indexes)?;
        let output = render_modification_result(
            "Deleted",
            &result.affected_pads,
            &result.messages,
            ctx.output_mode,
        );
        print!("{}", output);
    }
    Ok(())
}

fn handle_restore(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.restore_pads(ctx.scope, &indexes)?;
    let output = render_modification_result(
        "Restored",
        &result.affected_pads,
        &result.messages,
        ctx.output_mode,
    );
    print!("{}", output);
    Ok(())
}

fn handle_pin(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.pin_pads(ctx.scope, &indexes)?;
    let output = render_modification_result(
        "Pinned",
        &result.affected_pads,
        &result.messages,
        ctx.output_mode,
    );
    print!("{}", output);
    Ok(())
}

fn handle_unpin(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.unpin_pads(ctx.scope, &indexes)?;
    let output = render_modification_result(
        "Unpinned",
        &result.affected_pads,
        &result.messages,
        ctx.output_mode,
    );
    print!("{}", output);
    Ok(())
}

fn handle_complete(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.complete_pads(ctx.scope, &indexes)?;
    let output = render_modification_result(
        "Completed",
        &result.affected_pads,
        &result.messages,
        ctx.output_mode,
    );
    print!("{}", output);
    Ok(())
}

fn handle_move(ctx: &mut AppContext, mut indexes: Vec<String>, root: bool) -> Result<()> {
    let destination = if root {
        // If moving to root, all indexes are sources
        None
    } else {
        // Otherwise, last index is destination
        if indexes.len() < 2 {
            // Need at least source and dest
            // Actually, if indexes.len() == 1 and user expects "move 1 to root" they must use --root
            // We should clarify this in error message
            return Err(padzapp::error::PadzError::Api(
                "Missing destination. Use `padz move <SOURCE>... <DEST>` or `padz move <SOURCE>... --root`".to_string()
            ));
        }
        Some(indexes.pop().unwrap())
    };

    let result = ctx
        .api
        .move_pads(ctx.scope, &indexes, destination.as_deref())?;
    let output = render_modification_result(
        "Moved",
        &result.affected_pads,
        &result.messages,
        ctx.output_mode,
    );
    print!("{}", output);
    Ok(())
}

fn handle_reopen(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.reopen_pads(ctx.scope, &indexes)?;
    let output = render_modification_result(
        "Reopened",
        &result.affected_pads,
        &result.messages,
        ctx.output_mode,
    );
    print!("{}", output);
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

fn handle_paths(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.pad_paths(ctx.scope, &indexes)?;
    let lines: Vec<String> = result
        .pad_paths
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let output = render_text_list(&lines, "No pad paths found.", ctx.output_mode);
    print!("{}", output);
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_purge(
    ctx: &mut AppContext,
    indexes: Vec<String>,
    yes: bool,
    recursive: bool,
) -> Result<()> {
    // Pass confirmation flag directly to API
    // If not confirmed, API returns an error with a message about using --yes/-y
    let result = ctx.api.purge_pads(ctx.scope, &indexes, recursive, yes)?;
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_export(
    ctx: &mut AppContext,
    indexes: Vec<String>,
    single_file: Option<String>,
) -> Result<()> {
    let result = if let Some(title) = single_file {
        ctx.api
            .export_pads_single_file(ctx.scope, &indexes, &title)?
    } else {
        ctx.api.export_pads(ctx.scope, &indexes)?
    };
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_import(ctx: &mut AppContext, paths: Vec<String>) -> Result<()> {
    let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let result = ctx
        .api
        .import_pads(ctx.scope, paths, &ctx.import_extensions)?;
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_doctor(ctx: &mut AppContext) -> Result<()> {
    let result = ctx.api.doctor(ctx.scope)?;
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_config(ctx: &mut AppContext, key: Option<String>, value: Option<String>) -> Result<()> {
    let action = match (key.clone(), value) {
        (None, _) => ConfigAction::ShowAll,
        (Some(k), None) => ConfigAction::ShowKey(k),
        (Some(k), Some(v)) => ConfigAction::Set(k, v),
    };

    let result = ctx.api.config(ctx.scope, action)?;
    let mut lines = Vec::new();

    // If showing all, we need to iterate available keys manually since we don't have an iterator yet.
    // Or we just show known keys.
    if let Some(config) = &result.config {
        // If specific key was requested, show just that (handled by messages mostly,
        // but let's see what result.config has).
        // If action was ShowAll, we show everything.
        // If action was ShowKey, API might return config but messages have the info.

        if key.is_none() {
            // Show all known keys
            for (k, v) in config.list_all() {
                lines.push(format!("{} = {}", k, v));
            }
        }
    }
    let output = render_text_list(&lines, "No configuration values.", ctx.output_mode);
    print!("{}", output);
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_init(ctx: &AppContext) -> Result<()> {
    let result = ctx.api.init(ctx.scope)?;
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

// --- Tag management commands ---

fn handle_tags_list(ctx: &mut AppContext) -> Result<()> {
    let result = ctx.api.list_tags(ctx.scope)?;
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_tags_create(ctx: &mut AppContext, name: String) -> Result<()> {
    let result = ctx.api.create_tag(ctx.scope, &name)?;
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_tags_delete(ctx: &mut AppContext, name: String) -> Result<()> {
    let result = ctx.api.delete_tag(ctx.scope, &name)?;
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

fn handle_tags_rename(ctx: &mut AppContext, old_name: String, new_name: String) -> Result<()> {
    let result = ctx.api.rename_tag(ctx.scope, &old_name, &new_name)?;
    print_messages(&result.messages, ctx.output_mode);
    Ok(())
}

// --- Pad tagging commands ---

fn handle_add_tag(ctx: &mut AppContext, indexes: Vec<String>, tags: Vec<String>) -> Result<()> {
    let result = ctx.api.add_tags_to_pads(ctx.scope, &indexes, &tags)?;
    let output = render_modification_result(
        "Tagged",
        &result.affected_pads,
        &result.messages,
        ctx.output_mode,
    );
    print!("{}", output);
    Ok(())
}

fn handle_remove_tag(ctx: &mut AppContext, indexes: Vec<String>, tags: Vec<String>) -> Result<()> {
    let result = if tags.is_empty() {
        // If no tags specified, clear all tags from pads
        ctx.api.clear_tags_from_pads(ctx.scope, &indexes)?
    } else {
        // Remove specific tags
        ctx.api.remove_tags_from_pads(ctx.scope, &indexes, &tags)?
    };
    let output = render_modification_result(
        "Untagged",
        &result.affected_pads,
        &result.messages,
        ctx.output_mode,
    );
    print!("{}", output);
    Ok(())
}
