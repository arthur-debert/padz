use chrono::Utc;
use clap::Parser;
use colored::*;
use directories::ProjectDirs;
use padz::api::PadzApi;
use padz::clipboard::{copy_to_clipboard, format_for_clipboard};
use padz::config::PadzConfig;
use padz::editor::{EditorContent, edit_content};
use padz::index::{DisplayIndex, DisplayPad};
use padz::model::Scope;
use padz::store::fs::FileStore;
use std::path::PathBuf;
use unicode_width::UnicodeWidthStr;

mod args;
use args::{Cli, Commands};
use padz::error::Result;

fn main() {
    if let Err(e) = run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();

    // Determine Paths
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_padz_dir = cwd.join(".padz");

    // Global root: Use XDG
    let proj_dirs =
        ProjectDirs::from("com", "padz", "padz").expect("Could not determine config dir");
    let global_data_dir = proj_dirs.data_dir().to_path_buf();

    let scope = if cli.global {
        Scope::Global
    } else {
        Scope::Project
    };

    // Load config from the appropriate .padz directory
    let config_dir = match scope {
        Scope::Project => &project_padz_dir,
        Scope::Global => &global_data_dir,
    };
    let config = PadzConfig::load(config_dir).unwrap_or_default();
    let file_ext = config.get_file_ext().to_string();

    let store = FileStore::new(Some(project_padz_dir.clone()), global_data_dir.clone())
        .with_file_ext(&file_ext);

    let mut api = PadzApi::new(store);

    // Handle commands
    match cli.command {
        Some(Commands::Create {
            title,
            content,
            no_editor,
        }) => {
            let (final_title, final_content) = if no_editor {
                // No editor mode: require title
                let t = title.unwrap_or_default();
                let c = content.unwrap_or_default();
                (t, c)
            } else {
                // Editor mode
                let initial_title = title.unwrap_or_default();
                let initial_content = content.unwrap_or_default();
                let initial = EditorContent::new(initial_title, initial_content);

                match edit_content(&initial, &file_ext) {
                    Ok(edited) => (edited.title, edited.content),
                    Err(e) => {
                        eprintln!("Editor error: {}", e);
                        std::process::exit(1);
                    }
                }
            };

            if final_title.is_empty() {
                eprintln!("Error: Title cannot be empty");
                std::process::exit(1);
            }

            match api.create_pad(final_title, final_content, scope) {
                Ok(pad) => {
                    println!("Pad created: {}", pad.metadata.title.green());
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(Commands::List { search, deleted }) => {
            // If search arg is present, use search
            let result = if let Some(term) = search {
                api.search_pads(&term, scope)
            } else {
                api.list_pads(scope)
            };

            match result {
                Ok(pads) => {
                    // Filter: if NOT deleted flag, hide DisplayIndex::Deleted
                    let filtered: Vec<DisplayPad> = pads
                        .into_iter()
                        .filter(|p| {
                            if deleted {
                                matches!(p.index, DisplayIndex::Deleted(_))
                            } else {
                                !matches!(p.index, DisplayIndex::Deleted(_))
                            }
                        })
                        .collect();

                    print_pads(&filtered);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(Commands::View { indexes }) => {
            let idxs = parse_indexes(&indexes);
            let uuids = api.resolve_indexes(&idxs, scope)?;
            let pads = api.get_pads_by_uuids(&uuids, &idxs, scope)?;

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
            Ok(())
        }
        Some(Commands::Edit { indexes }) => {
            let idxs = parse_indexes(&indexes);
            let uuids = api.resolve_indexes(&idxs, scope)?;
            let pads = api.get_pads_by_uuids(&uuids, &idxs, scope)?;

            for (uuid, dp) in uuids.iter().zip(pads.iter()) {
                let initial =
                    EditorContent::new(dp.pad.metadata.title.clone(), dp.pad.content.clone());

                match edit_content(&initial, &file_ext) {
                    Ok(edited) => {
                        if edited.title.is_empty() {
                            eprintln!("Error: Title cannot be empty");
                            std::process::exit(1);
                        }

                        // Update using UUID directly
                        let pad = api.update_pad_by_uuid(
                            uuid,
                            edited.title.clone(),
                            edited.content.clone(),
                            scope,
                        )?;

                        // Copy to clipboard on exit
                        let clipboard_text = format_for_clipboard(&edited.title, &edited.content);
                        if let Err(e) = copy_to_clipboard(&clipboard_text) {
                            eprintln!("Warning: Failed to copy to clipboard: {}", e);
                        }

                        println!("Pad updated: {}", pad.metadata.title.green());
                    }
                    Err(e) => {
                        eprintln!("Editor error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Ok(())
        }
        Some(Commands::Open { indexes }) => {
            let idxs = parse_indexes(&indexes);
            let uuids = api.resolve_indexes(&idxs, scope)?;
            let pads = api.get_pads_by_uuids(&uuids, &idxs, scope)?;

            for (uuid, dp) in uuids.iter().zip(pads.iter()) {
                let initial =
                    EditorContent::new(dp.pad.metadata.title.clone(), dp.pad.content.clone());

                match edit_content(&initial, &file_ext) {
                    Ok(edited) => {
                        // Copy to clipboard on exit (even if content unchanged)
                        let clipboard_text = format_for_clipboard(&edited.title, &edited.content);
                        if let Err(e) = copy_to_clipboard(&clipboard_text) {
                            eprintln!("Warning: Failed to copy to clipboard: {}", e);
                        }

                        // Only save if content changed
                        if edited.title != dp.pad.metadata.title || edited.content != dp.pad.content
                        {
                            if edited.title.is_empty() {
                                eprintln!("Error: Title cannot be empty");
                                std::process::exit(1);
                            }

                            let pad =
                                api.update_pad_by_uuid(uuid, edited.title, edited.content, scope)?;
                            println!("Pad updated: {}", pad.metadata.title.green());
                        } else {
                            println!("Pad content copied to clipboard.");
                        }
                    }
                    Err(e) => {
                        eprintln!("Editor error: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Ok(())
        }
        Some(Commands::Delete { indexes }) => {
            let idxs = parse_indexes(&indexes);
            let uuids = api.resolve_indexes(&idxs, scope)?;
            let deleted = api.delete_pads_by_uuids(&uuids, scope)?;

            for pad in &deleted {
                println!("Pad deleted: {}", pad.metadata.title.red());
            }
            Ok(())
        }
        Some(Commands::Pin { indexes }) => {
            let idxs = parse_indexes(&indexes);
            let uuids = api.resolve_indexes(&idxs, scope)?;
            let pinned = api.pin_pads_by_uuids(&uuids, scope)?;

            for pad in &pinned {
                println!("Pad pinned: {}", pad.metadata.title.yellow());
            }
            Ok(())
        }
        Some(Commands::Unpin { indexes }) => {
            let idxs = parse_indexes(&indexes);
            let uuids = api.resolve_indexes(&idxs, scope)?;
            let unpinned = api.unpin_pads_by_uuids(&uuids, scope)?;

            for pad in &unpinned {
                println!("Pad unpinned: {}", pad.metadata.title);
            }
            Ok(())
        }
        Some(Commands::Search { term }) => match api.search_pads(&term, scope) {
            Ok(pads) => {
                print_pads(&pads);
                Ok(())
            }
            Err(e) => Err(e),
        },
        Some(Commands::Path { indexes }) => {
            let idxs = parse_indexes(&indexes);
            let uuids = api.resolve_indexes(&idxs, scope)?;
            let paths = api.get_pad_paths_by_uuids(&uuids, scope)?;

            for path in &paths {
                println!("{}", path.display());
            }
            Ok(())
        }
        Some(Commands::Config { key, value }) => {
            let mut config = PadzConfig::load(config_dir).unwrap_or_default();

            match (key.as_deref(), value) {
                // No key: show all config
                (None, _) => {
                    println!("file-ext = {}", config.get_file_ext());
                    Ok(())
                }
                // Key without value: show that key
                (Some("file-ext"), None) => {
                    println!("{}", config.get_file_ext());
                    Ok(())
                }
                // Key with value: set it
                (Some("file-ext"), Some(v)) => {
                    config.set_file_ext(&v);
                    match config.save(config_dir) {
                        Ok(()) => {
                            println!("file-ext set to {}", config.get_file_ext());
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                // Unknown key
                (Some(k), _) => {
                    eprintln!("Unknown config key: {}", k);
                    eprintln!("Available keys: file-ext");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Init) => {
            println!("Initialized padz store");
            Ok(())
        }
        None => {
            // Default to List
            let pads = api.list_pads(scope)?;
            print_pads(&pads);
            Ok(())
        }
    }
}

fn parse_index(s: &str) -> DisplayIndex {
    // Basic parsing: p1 -> Pinned(1), d1 -> Deleted(1), 1 -> Regular(1)
    if let Some(rest) = s.strip_prefix('p')
        && let Ok(n) = rest.parse()
    {
        return DisplayIndex::Pinned(n);
    }
    if let Some(rest) = s.strip_prefix('d')
        && let Ok(n) = rest.parse()
    {
        return DisplayIndex::Deleted(n);
    }
    if let Ok(n) = s.parse() {
        return DisplayIndex::Regular(n);
    }
    // Fallback or panic? For CLI better to error properly, but for now simple panic or default
    eprintln!("Invalid index format: {}", s);
    std::process::exit(1);
}

fn parse_indexes(strs: &[String]) -> Vec<DisplayIndex> {
    strs.iter().map(|s| parse_index(s)).collect()
}

const LINE_WIDTH: usize = 100;
const TIME_WIDTH: usize = 14; // "XX minutes ago" or "XX hours  ago"
const PIN_MARKER: &str = "⚲";

/// Truncate a string to fit within a given display width, adding "…" if truncated.
fn truncate_to_width(s: &str, max_width: usize) -> String {
    use unicode_width::UnicodeWidthChar;

    let mut result = String::new();
    let mut current_width = 0;

    for c in s.chars() {
        let char_width = c.width().unwrap_or(0);
        if current_width + char_width > max_width.saturating_sub(1) {
            // Need room for ellipsis
            result.push('…');
            return result;
        }
        result.push(c);
        current_width += char_width;
    }

    // No truncation needed
    result
}

fn format_time_ago(timestamp: chrono::DateTime<Utc>) -> String {
    let now = Utc::now();
    let duration = now.signed_duration_since(timestamp);

    let formatter = timeago::Formatter::new();
    let time_str = formatter.convert(duration.to_std().unwrap_or_default());

    // Add extra space for "hour ago" -> "hour  ago" to match "hours ago" length
    let time_str = time_str
        .replace("hour ago", "hour  ago")
        .replace("minute ago", "minute  ago")
        .replace("second ago", "second  ago")
        .replace("day ago", "day  ago")
        .replace("week ago", "week  ago")
        .replace("month ago", "month  ago")
        .replace("year ago", "year  ago");

    // Right-align to TIME_WIDTH
    format!("{:>width$}", time_str, width = TIME_WIDTH)
}

fn print_pads(pads: &[DisplayPad]) {
    if pads.is_empty() {
        println!("No pads found.");
        return;
    }

    // Print blank line before pinned section if there are pinned items
    let has_pinned = pads
        .iter()
        .any(|dp| matches!(dp.index, DisplayIndex::Pinned(_)));
    if has_pinned {
        println!();
    }

    let mut last_was_pinned = false;
    for dp in pads {
        let is_pinned_entry = matches!(dp.index, DisplayIndex::Pinned(_));

        // Add blank line when transitioning from pinned to regular
        if last_was_pinned && !is_pinned_entry {
            println!();
        }
        last_was_pinned = is_pinned_entry;

        // Format index: "p1. " or "1. " or "d1. "
        let idx_str = match &dp.index {
            DisplayIndex::Pinned(n) => format!("p{}. ", n),
            DisplayIndex::Regular(n) => format!("{}. ", n),
            DisplayIndex::Deleted(n) => format!("d{}. ", n),
        };

        // Left prefix: "  ⚲ " for pinned, "    " for regular
        let left_prefix = if is_pinned_entry {
            format!("  {} ", PIN_MARKER)
        } else {
            "    ".to_string()
        };
        let left_prefix_width = left_prefix.width();

        // Right suffix: "⚲ " if pad is pinned (shown in regular list too), "  " otherwise
        let right_suffix = if dp.pad.metadata.is_pinned && !is_pinned_entry {
            format!("{} ", PIN_MARKER)
        } else {
            "  ".to_string()
        };
        let right_suffix_width = right_suffix.width();

        // Time ago string (already right-padded to TIME_WIDTH)
        let time_ago = format_time_ago(dp.pad.metadata.created_at);

        // Build title + content preview
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

        // Calculate available space for title_content
        // Format: "{left_prefix}{idx_str}{title_content}{padding}{right_suffix}{time_ago}"
        let idx_width = idx_str.width();
        let fixed_width = left_prefix_width + idx_width + right_suffix_width + TIME_WIDTH;
        let available = LINE_WIDTH.saturating_sub(fixed_width);

        // Truncate title_content to fit available display width
        let title_display: String = truncate_to_width(&title_content, available);

        // Build the full line with proper padding
        let content_width = title_display.width();
        let padding = available.saturating_sub(content_width);

        // Color the output
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
