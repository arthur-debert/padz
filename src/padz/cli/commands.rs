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
    print_messages, render_full_pads, render_pad_list, render_pad_list_deleted, render_text_list,
};
use super::setup::{
    print_grouped_help, print_help_for_command, print_subcommand_help, Cli, Commands,
    CompletionShell, CoreCommands, DataCommands, MiscCommands, PadCommands,
};
use clap::Parser;
use padz::api::{ConfigAction, PadFilter, PadStatusFilter, PadzApi};
use padz::clipboard::{copy_to_clipboard, format_for_clipboard, get_from_clipboard};
use padz::editor::open_in_editor;
use padz::error::Result;
use padz::init::initialize;
use padz::model::Scope;
use padz::model::{extract_title_and_body, parse_pad_content};
use padz::store::fs::FileStore;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

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
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    // Handle help flag - at top level use grouped help, for subcommands use clap's default
    if cli.help {
        if cli.command.is_none() {
            print_grouped_help();
        } else {
            print_subcommand_help(&cli.command);
        }
        return Ok(());
    }

    // Handle completions before context init (they don't need API)
    if let Some(Commands::Misc(MiscCommands::Completions { shell })) = &cli.command {
        return handle_completions(*shell);
    }

    let mut ctx = init_context(&cli)?;

    match cli.command {
        Some(Commands::Core(cmd)) => match cmd {
            CoreCommands::Create { title, no_editor } => {
                // Join all title words with spaces
                let title = if title.is_empty() {
                    None
                } else {
                    Some(title.join(" "))
                };
                handle_create(&mut ctx, title, no_editor)
            }
            CoreCommands::List {
                search,
                deleted,
                peek,
            } => handle_list(&mut ctx, search, deleted, peek),
            CoreCommands::Search { term } => handle_search(&mut ctx, term),
        },
        Some(Commands::Pad(cmd)) => match cmd {
            PadCommands::View { indexes, peek } => handle_view(&mut ctx, indexes, peek),
            PadCommands::Edit { indexes } => handle_edit(&mut ctx, indexes),
            PadCommands::Open { indexes } => handle_open(&mut ctx, indexes),
            PadCommands::Delete { indexes } => handle_delete(&mut ctx, indexes),
            PadCommands::Restore { indexes } => handle_restore(&mut ctx, indexes),
            PadCommands::Pin { indexes } => handle_pin(&mut ctx, indexes),
            PadCommands::Unpin { indexes } => handle_unpin(&mut ctx, indexes),
            PadCommands::Path { indexes } => handle_paths(&mut ctx, indexes),
        },
        Some(Commands::Data(cmd)) => match cmd {
            DataCommands::Purge { indexes, yes } => handle_purge(&mut ctx, indexes, yes),
            DataCommands::Export {
                single_file,
                indexes,
            } => handle_export(&mut ctx, indexes, single_file),
            DataCommands::Import { paths } => handle_import(&mut ctx, paths),
        },
        Some(Commands::Misc(cmd)) => match cmd {
            MiscCommands::Doctor => handle_doctor(&mut ctx),
            MiscCommands::Config { key, value } => handle_config(&mut ctx, key, value),
            MiscCommands::Init => handle_init(&ctx),
            MiscCommands::Help { command } => handle_help(command),
            MiscCommands::Completions { shell } => handle_completions(shell),
            MiscCommands::CompletePads { deleted } => handle_complete_pads(&mut ctx, deleted),
        },
        None => handle_list(&mut ctx, None, false, false),
    }
}

fn init_context(cli: &Cli) -> Result<AppContext> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let ctx = initialize(&cwd, cli.global);

    Ok(AppContext {
        api: ctx.api,
        scope: ctx.scope,
        import_extensions: ctx.config.import_extensions.clone(),
    })
}

fn handle_create(ctx: &mut AppContext, title: Option<String>, no_editor: bool) -> Result<()> {
    let mut final_title = title;
    let mut initial_content = String::new();
    let mut should_open_editor = !no_editor;

    // 1. Check for piped input (stdin)
    if !std::io::stdin().is_terminal() {
        let mut buffer = String::new();
        // Read stdin content
        if std::io::stdin().read_to_string(&mut buffer).is_ok() && !buffer.trim().is_empty() {
            // If pipe has content, we use it.
            // If no title was provided in args, we extract it from the content.
            if final_title.is_none() {
                if let Some((parsed_title, _)) = parse_pad_content(&buffer) {
                    final_title = Some(parsed_title);
                }
            }
            initial_content = buffer;
            // Piped input skips the editor by default, as per requirement
            should_open_editor = false;
        }
    }

    // 2. If still no content/title, check clipboard
    // "In case it's called with no text as the padz content as argument, use the clipboard data as the padz content."
    if final_title.is_none() && initial_content.is_empty() {
        if let Ok(clipboard_content) = get_from_clipboard() {
            if !clipboard_content.trim().is_empty() {
                // Parse title from clipboard content
                if let Some((parsed_title, _)) = parse_pad_content(&clipboard_content) {
                    final_title = Some(parsed_title);
                }
                initial_content = clipboard_content;
                // Clipboard creation preserves editor behavior (opens unless --no-editor)
            }
        }
    }

    // Use provided/parsed title or "Untitled" as placeholder
    let title_to_use = final_title.unwrap_or_else(|| "Untitled".to_string());

    let result = ctx
        .api
        .create_pad(ctx.scope, title_to_use, initial_content)?;
    print_messages(&result.messages);

    // Open editor if requested/appropriate
    if should_open_editor && !result.affected_pads.is_empty() {
        let pad = &result.affected_pads[0];
        let path = ctx.api.get_path_by_id(ctx.scope, pad.metadata.id)?;
        open_in_editor(&path)?;

        // Re-read the pad after editing and copy to clipboard
        copy_pad_to_clipboard(&path);
    }

    Ok(())
}

fn handle_list(
    ctx: &mut AppContext,
    search: Option<String>,
    deleted: bool,
    peek: bool,
) -> Result<()> {
    let filter = if let Some(term) = search {
        PadFilter {
            status: if deleted {
                PadStatusFilter::Deleted
            } else {
                PadStatusFilter::Active
            },
            search_term: Some(term),
        }
    } else {
        PadFilter {
            status: if deleted {
                PadStatusFilter::Deleted
            } else {
                PadStatusFilter::Active
            },
            search_term: None,
        }
    };

    let result = ctx.api.get_pads(ctx.scope, filter)?;

    // Use outstanding-based rendering
    let output = if deleted {
        render_pad_list_deleted(&result.listed_pads, peek)
    } else {
        render_pad_list(&result.listed_pads, peek)
    };
    print!("{}", output);

    print_messages(&result.messages);
    Ok(())
}

fn handle_view(ctx: &mut AppContext, indexes: Vec<String>, peek: bool) -> Result<()> {
    let result = ctx.api.view_pads(ctx.scope, &indexes)?;
    let output = if peek {
        // Reuse list rendering for peek view
        render_pad_list(&result.listed_pads, true)
    } else {
        render_full_pads(&result.listed_pads)
    };
    print!("{}", output);
    print_messages(&result.messages);

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
    let result = ctx.api.view_pads(ctx.scope, &indexes)?;

    for dp in &result.listed_pads {
        let path = ctx.api.get_path_by_id(ctx.scope, dp.pad.metadata.id)?;
        open_in_editor(&path)?;

        // Re-read the pad after editing and copy to clipboard
        copy_pad_to_clipboard(&path);
    }

    Ok(())
}

fn handle_open(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    // Open behaves exactly like edit now - just open the file.
    // The "sync only if changed" logic is handled by the lazy reconciler (padz list).
    handle_edit(ctx, indexes)
}

fn handle_delete(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.delete_pads(ctx.scope, &indexes)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_restore(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.restore_pads(ctx.scope, &indexes)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_pin(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.pin_pads(ctx.scope, &indexes)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_unpin(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.unpin_pads(ctx.scope, &indexes)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_search(ctx: &mut AppContext, term: String) -> Result<()> {
    let filter = PadFilter {
        status: PadStatusFilter::Active,
        search_term: Some(term),
    };
    let result = ctx.api.get_pads(ctx.scope, filter)?;
    let output = render_pad_list(&result.listed_pads, false);
    print!("{}", output);
    print_messages(&result.messages);
    Ok(())
}

fn handle_paths(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.pad_paths(ctx.scope, &indexes)?;
    let lines: Vec<String> = result
        .pad_paths
        .iter()
        .map(|path| path.display().to_string())
        .collect();
    let output = render_text_list(&lines, "No pad paths found.");
    print!("{}", output);
    print_messages(&result.messages);
    Ok(())
}

fn handle_purge(ctx: &mut AppContext, indexes: Vec<String>, yes: bool) -> Result<()> {
    let result = ctx.api.purge_pads(ctx.scope, &indexes, yes)?;
    print_messages(&result.messages);
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
    print_messages(&result.messages);
    Ok(())
}

fn handle_import(ctx: &mut AppContext, paths: Vec<String>) -> Result<()> {
    let paths: Vec<PathBuf> = paths.iter().map(PathBuf::from).collect();
    let result = ctx
        .api
        .import_pads(ctx.scope, paths, &ctx.import_extensions)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_doctor(ctx: &mut AppContext) -> Result<()> {
    let result = ctx.api.doctor(ctx.scope)?;
    print_messages(&result.messages);
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
    let output = render_text_list(&lines, "No configuration values.");
    print!("{}", output);
    print_messages(&result.messages);
    Ok(())
}

fn handle_init(ctx: &AppContext) -> Result<()> {
    let result = ctx.api.init(ctx.scope)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_help(command: Option<String>) -> Result<()> {
    match command {
        Some(cmd) => print_help_for_command(&cmd),
        None => print_grouped_help(),
    }
    Ok(())
}

fn handle_completions(shell: CompletionShell) -> Result<()> {
    match shell {
        CompletionShell::Bash => print!("{}", BASH_COMPLETION_SCRIPT),
        CompletionShell::Zsh => print!("{}", ZSH_COMPLETION_SCRIPT),
    }
    Ok(())
}

fn handle_complete_pads(ctx: &mut AppContext, include_deleted: bool) -> Result<()> {
    let mut entries = Vec::new();

    let filter = PadFilter {
        status: if include_deleted {
            PadStatusFilter::All
        } else {
            PadStatusFilter::Active
        },
        search_term: None,
    };

    let result = ctx.api.get_pads(ctx.scope, filter)?;

    for dp in result.listed_pads {
        entries.push((dp.index.to_string(), dp.pad.metadata.title));
    }

    for (index, title) in entries {
        println!("{}	{}", index, title);
    }

    Ok(())
}

const BASH_COMPLETION_SCRIPT: &str = include_str!("bash.completion.sh");
const ZSH_COMPLETION_SCRIPT: &str = include_str!("z-completion.sh");
