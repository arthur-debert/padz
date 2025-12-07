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

use super::print::print_messages;
use super::render::{render_full_pads, render_pad_list, render_text_list};
use super::setup::{
    print_grouped_help, print_help_for_command, print_subcommand_help, Cli, Commands,
    CompletionShell, CoreCommands, DataCommands, MiscCommands, PadCommands,
};
use clap::Parser;
use directories::ProjectDirs;
use padz::api::{ConfigAction, PadFilter, PadStatusFilter, PadUpdate, PadzApi, PadzPaths};
use padz::clipboard::{copy_to_clipboard, format_for_clipboard};
use padz::config::PadzConfig;
use padz::editor::{edit_content, EditorContent};
use padz::error::{PadzError, Result};
use padz::model::Scope;
use padz::store::fs::FileStore;
use std::path::PathBuf;

struct AppContext {
    api: PadzApi<FileStore>,
    scope: Scope,
    file_ext: String,
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
            CoreCommands::Create {
                title,
                content,
                no_editor,
            } => handle_create(&mut ctx, title, content, no_editor),
            CoreCommands::List { search, deleted } => handle_list(&mut ctx, search, deleted),
            CoreCommands::Search { term } => handle_search(&mut ctx, term),
        },
        Some(Commands::Pad(cmd)) => match cmd {
            PadCommands::View { indexes } => handle_view(&mut ctx, indexes),
            PadCommands::Edit { indexes } => handle_edit(&mut ctx, indexes),
            PadCommands::Open { indexes } => handle_open(&mut ctx, indexes),
            PadCommands::Delete { indexes } => handle_delete(&mut ctx, indexes),
            PadCommands::Pin { indexes } => handle_pin(&mut ctx, indexes),
            PadCommands::Unpin { indexes } => handle_unpin(&mut ctx, indexes),
            PadCommands::Path { indexes } => handle_paths(&mut ctx, indexes),
        },
        Some(Commands::Data(cmd)) => match cmd {
            DataCommands::Purge { indexes, yes } => handle_purge(&mut ctx, indexes, yes),
            DataCommands::Export { indexes } => handle_export(&mut ctx, indexes),
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
        None => handle_list(&mut ctx, None, false),
    }
}

fn init_context(cli: &Cli) -> Result<AppContext> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_padz_dir = cwd.join(".padz");

    let proj_dirs =
        ProjectDirs::from("com", "padz", "padz").expect("Could not determine config dir");
    let global_data_dir = proj_dirs.data_dir().to_path_buf();

    let scope = if cli.global {
        Scope::Global
    } else {
        Scope::Project
    };

    let config_dir = match scope {
        Scope::Project => &project_padz_dir,
        Scope::Global => &global_data_dir,
    };
    let config = PadzConfig::load(config_dir).unwrap_or_default();
    let file_ext = config.get_file_ext().to_string();
    let import_extensions = config.import_extensions.clone();

    let store = FileStore::new(Some(project_padz_dir.clone()), global_data_dir.clone())
        .with_file_ext(&file_ext);
    let paths = PadzPaths {
        project: Some(project_padz_dir),
        global: global_data_dir,
    };
    let api = PadzApi::new(store, paths);

    Ok(AppContext {
        api,
        scope,
        file_ext,
        import_extensions,
    })
}

fn handle_create(
    ctx: &mut AppContext,
    title: Option<String>,
    content: Option<String>,
    no_editor: bool,
) -> Result<()> {
    let (final_title, final_content) = if no_editor {
        (title.unwrap_or_default(), content.unwrap_or_default())
    } else {
        let initial = EditorContent::new(title.unwrap_or_default(), content.unwrap_or_default());
        let edited = edit_content(&initial, &ctx.file_ext)?;
        (edited.title, edited.content)
    };

    if final_title.is_empty() {
        return Err(PadzError::Api("Title cannot be empty".into()));
    }

    let result = ctx.api.create_pad(ctx.scope, final_title, final_content)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_list(ctx: &mut AppContext, search: Option<String>, deleted: bool) -> Result<()> {
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
    let output = render_pad_list(&result.listed_pads);
    print!("{}", output);

    print_messages(&result.messages);
    Ok(())
}

fn handle_view(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.view_pads(ctx.scope, &indexes)?;
    let output = render_full_pads(&result.listed_pads);
    print!("{}", output);
    print_messages(&result.messages);
    Ok(())
}

fn handle_edit(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.view_pads(ctx.scope, &indexes)?;

    let mut updates = Vec::new();
    for dp in &result.listed_pads {
        let initial = EditorContent::from_buffer(&dp.pad.content);
        let edited = edit_content(&initial, &ctx.file_ext)?;
        if edited.title.is_empty() {
            return Err(PadzError::Api("Title cannot be empty".into()));
        }

        let clipboard_text = format_for_clipboard(&edited.title, &edited.content);
        if let Err(e) = copy_to_clipboard(&clipboard_text) {
            eprintln!("Warning: Failed to copy to clipboard: {}", e);
        }

        updates.push(PadUpdate::new(
            dp.index.clone(),
            edited.title.clone(),
            edited.content.clone(),
        ));
    }

    if updates.is_empty() {
        return Ok(());
    }

    let result = ctx.api.update_pads(ctx.scope, &updates)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_open(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.view_pads(ctx.scope, &indexes)?;

    let mut updates = Vec::new();
    for dp in &result.listed_pads {
        let initial = EditorContent::from_buffer(&dp.pad.content);
        let edited = edit_content(&initial, &ctx.file_ext)?;

        let clipboard_text = format_for_clipboard(&edited.title, &edited.content);
        if let Err(e) = copy_to_clipboard(&clipboard_text) {
            eprintln!("Warning: Failed to copy to clipboard: {}", e);
        }

        if edited.title != dp.pad.metadata.title || edited.content != dp.pad.content {
            if edited.title.is_empty() {
                return Err(PadzError::Api("Title cannot be empty".into()));
            }
            updates.push(PadUpdate::new(
                dp.index.clone(),
                edited.title.clone(),
                edited.content.clone(),
            ));
        }
    }

    if updates.is_empty() {
        return Ok(());
    }

    let result = ctx.api.update_pads(ctx.scope, &updates)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_delete(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.delete_pads(ctx.scope, &indexes)?;
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
    let output = render_pad_list(&result.listed_pads);
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

fn handle_export(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let result = ctx.api.export_pads(ctx.scope, &indexes)?;
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
            if let Some(val) = config.get("file-ext") {
                lines.push(format!("file-ext = {}", val));
            }
            if let Some(val) = config.get("import-extensions") {
                lines.push(format!("import-extensions = {}", val));
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
