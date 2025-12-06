use chrono::Utc;
use clap::Parser;
use colored::*;
use directories::ProjectDirs;
use padz::api::{CmdMessage, ConfigAction, MessageLevel, PadUpdate, PadzApi, PadzPaths};
use padz::clipboard::{copy_to_clipboard, format_for_clipboard};
use padz::config::PadzConfig;
use padz::editor::{EditorContent, edit_content};
use padz::error::{PadzError, Result};
use padz::index::{DisplayIndex, DisplayPad};
use padz::model::Scope;
use padz::store::fs::FileStore;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

mod args;
use args::{Cli, Commands};

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

struct AppContext {
    api: PadzApi<FileStore>,
    scope: Scope,
    file_ext: String,
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let mut ctx = init_context(&cli)?;

    match cli.command {
        Some(Commands::Create {
            title,
            content,
            no_editor,
        }) => handle_create(&mut ctx, title, content, no_editor),
        Some(Commands::List { search, deleted }) => handle_list(&mut ctx, search, deleted),
        Some(Commands::View { indexes }) => handle_view(&mut ctx, indexes),
        Some(Commands::Edit { indexes }) => handle_edit(&mut ctx, indexes),
        Some(Commands::Open { indexes }) => handle_open(&mut ctx, indexes),
        Some(Commands::Delete { indexes }) => handle_delete(&mut ctx, indexes),
        Some(Commands::Pin { indexes }) => handle_pin(&mut ctx, indexes),
        Some(Commands::Unpin { indexes }) => handle_unpin(&mut ctx, indexes),
        Some(Commands::Search { term }) => handle_search(&mut ctx, term),
        Some(Commands::Path { indexes }) => handle_paths(&mut ctx, indexes),
        Some(Commands::Config { key, value }) => handle_config(&mut ctx, key, value),
        Some(Commands::Init) => handle_init(&ctx),
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
    let result = if let Some(term) = search {
        ctx.api.search_pads(ctx.scope, &term)?
    } else {
        ctx.api.list_pads(ctx.scope, deleted)?
    };
    print_pads(&result.listed_pads);
    print_messages(&result.messages);
    Ok(())
}

fn handle_view(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let parsed = parse_indexes(&indexes)?;
    let result = ctx.api.view_pads(ctx.scope, &parsed)?;
    print_full_pads(&result.listed_pads);
    print_messages(&result.messages);
    Ok(())
}

fn handle_edit(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let parsed = parse_indexes(&indexes)?;
    let result = ctx.api.view_pads(ctx.scope, &parsed)?;

    let mut updates = Vec::new();
    for dp in &result.listed_pads {
        let initial = EditorContent::new(dp.pad.metadata.title.clone(), dp.pad.content.clone());
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
    let parsed = parse_indexes(&indexes)?;
    let result = ctx.api.view_pads(ctx.scope, &parsed)?;

    let mut updates = Vec::new();
    for dp in &result.listed_pads {
        let initial = EditorContent::new(dp.pad.metadata.title.clone(), dp.pad.content.clone());
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
        } else {
            println!("Pad content copied to clipboard.");
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
    let parsed = parse_indexes(&indexes)?;
    let result = ctx.api.delete_pads(ctx.scope, &parsed)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_pin(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let parsed = parse_indexes(&indexes)?;
    let result = ctx.api.pin_pads(ctx.scope, &parsed)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_unpin(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let parsed = parse_indexes(&indexes)?;
    let result = ctx.api.unpin_pads(ctx.scope, &parsed)?;
    print_messages(&result.messages);
    Ok(())
}

fn handle_search(ctx: &mut AppContext, term: String) -> Result<()> {
    let result = ctx.api.search_pads(ctx.scope, &term)?;
    print_pads(&result.listed_pads);
    print_messages(&result.messages);
    Ok(())
}

fn handle_paths(ctx: &mut AppContext, indexes: Vec<String>) -> Result<()> {
    let parsed = parse_indexes(&indexes)?;
    let result = ctx.api.pad_paths(ctx.scope, &parsed)?;
    for path in &result.pad_paths {
        println!("{}", path.display());
    }
    print_messages(&result.messages);
    Ok(())
}

fn handle_config(ctx: &mut AppContext, key: Option<String>, value: Option<String>) -> Result<()> {
    let action = match (key.as_deref(), value) {
        (None, _) => ConfigAction::ShowAll,
        (Some("file-ext"), None) => ConfigAction::ShowKey("file-ext".to_string()),
        (Some("file-ext"), Some(v)) => ConfigAction::SetFileExt(v),
        (Some(other), _) => {
            println!("Unknown config key: {}", other);
            return Ok(());
        }
    };

    let result = ctx.api.config(ctx.scope, action)?;
    if let Some(config) = &result.config {
        println!("file-ext = {}", config.get_file_ext());
    }
    print_messages(&result.messages);
    Ok(())
}

fn handle_init(ctx: &AppContext) -> Result<()> {
    let result = ctx.api.init(ctx.scope)?;
    print_messages(&result.messages);
    Ok(())
}

fn print_messages(messages: &[CmdMessage]) {
    for message in messages {
        match message.level {
            MessageLevel::Info => println!("{}", message.content.dimmed()),
            MessageLevel::Success => println!("{}", message.content.green()),
            MessageLevel::Warning => println!("{}", message.content.yellow()),
            MessageLevel::Error => println!("{}", message.content.red()),
        }
    }
}

fn print_full_pads(pads: &[DisplayPad]) {
    for (i, dp) in pads.iter().enumerate() {
        if i > 0 {
            println!("\n================================\n");
        }
        println!(
            "{} {}",
            dp.index.to_string().yellow(),
            dp.pad.metadata.title.bold()
        );
        println!("--------------------------------");
        println!("{}", dp.pad.content);
    }
}

const LINE_WIDTH: usize = 100;
const TIME_WIDTH: usize = 14;
const PIN_MARKER: &str = "⚲";

fn print_pads(pads: &[DisplayPad]) {
    if pads.is_empty() {
        println!("No pads found.");
        return;
    }

    let has_pinned = pads
        .iter()
        .any(|dp| matches!(dp.index, DisplayIndex::Pinned(_)));
    if has_pinned {
        println!();
    }

    let mut last_was_pinned = false;
    for dp in pads {
        let is_pinned_entry = matches!(dp.index, DisplayIndex::Pinned(_));

        if last_was_pinned && !is_pinned_entry {
            println!();
        }
        last_was_pinned = is_pinned_entry;

        let idx_str = match &dp.index {
            DisplayIndex::Pinned(n) => format!("p{}. ", n),
            DisplayIndex::Regular(n) => format!("{}. ", n),
            DisplayIndex::Deleted(n) => format!("d{}. ", n),
        };

        let left_prefix = if is_pinned_entry {
            format!("  {} ", PIN_MARKER)
        } else {
            "    ".to_string()
        };
        let left_prefix_width = left_prefix.width();

        let right_suffix = if dp.pad.metadata.is_pinned && !is_pinned_entry {
            format!("{} ", PIN_MARKER)
        } else {
            "  ".to_string()
        };
        let right_suffix_width = right_suffix.width();

        let time_ago = format_time_ago(dp.pad.metadata.created_at);

        let title = &dp.pad.metadata.title;
        let content_preview: String = dp
            .pad
            .content
            .chars()
            .take(50)
            .map(|c| if c == '\n' { ' ' } else { c })
            .collect();
        let title_content = if content_preview.is_empty() {
            title.clone()
        } else {
            format!("{} {}", title, content_preview)
        };

        let idx_width = idx_str.width();
        let fixed_width = left_prefix_width + idx_width + right_suffix_width + TIME_WIDTH;
        let available = LINE_WIDTH.saturating_sub(fixed_width);

        let title_display: String = truncate_to_width(&title_content, available);

        let content_width = title_display.width();
        let padding = available.saturating_sub(content_width);

        let idx_colored = match dp.index {
            DisplayIndex::Pinned(_) => idx_str.yellow(),
            DisplayIndex::Deleted(_) => idx_str.red(),
            DisplayIndex::Regular(_) => idx_str.normal(),
        };

        let time_colored = time_ago.dimmed();

        println!(
            "{}{}{}{}{}{}",
            left_prefix,
            idx_colored,
            title_display,
            " ".repeat(padding),
            right_suffix,
            time_colored
        );
    }
}

fn truncate_to_width(s: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthChar;

    let mut result = String::new();
    let mut current_width = 0;

    for c in s.chars() {
        let char_width = c.width().unwrap_or(0);
        if current_width + char_width > max_width.saturating_sub(1) {
            result.push('…');
            return result;
        }
        result.push(c);
        current_width += char_width;
    }

    result
}

fn format_time_ago(timestamp: chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(timestamp);

    let formatter = timeago::Formatter::new();
    let time_str = formatter.convert(duration.to_std().unwrap_or_default());

    let time_str = time_str
        .replace("hour ago", "hour  ago")
        .replace("minute ago", "minute  ago")
        .replace("second ago", "second  ago")
        .replace("day ago", "day  ago")
        .replace("week ago", "week  ago")
        .replace("month ago", "month  ago")
        .replace("year ago", "year  ago");

    format!("{:>width$}", time_str, width = TIME_WIDTH)
}

fn parse_index(s: &str) -> Result<DisplayIndex> {
    if let Some(rest) = s.strip_prefix('p')
        && let Ok(n) = rest.parse()
    {
        return Ok(DisplayIndex::Pinned(n));
    }
    if let Some(rest) = s.strip_prefix('d')
        && let Ok(n) = rest.parse()
    {
        return Ok(DisplayIndex::Deleted(n));
    }
    if let Ok(n) = s.parse() {
        return Ok(DisplayIndex::Regular(n));
    }
    Err(PadzError::Api(format!("Invalid index format: {}", s)))
}

fn parse_indexes(strs: &[String]) -> Result<Vec<DisplayIndex>> {
    strs.iter().map(|s| parse_index(s)).collect()
}
