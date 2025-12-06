use clap::Parser;
use colored::*;
use directories::ProjectDirs;
use padz_lib::api::PadzApi;
use padz_lib::editor::{EditorContent, edit_content};
use padz_lib::index::{DisplayIndex, DisplayPad};
use padz_lib::model::Scope;
use padz_lib::store::fs::FileStore;
use std::path::PathBuf;

mod args;
use args::{Cli, Commands};

fn main() {
    let cli = Cli::parse();

    // Determine Paths
    // Project root: CWD if it has .git? Or pass it.
    // For now assuming CWD is project root.
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Global root: Use XDG
    let proj_dirs =
        ProjectDirs::from("com", "padz", "padz").expect("Could not determine config dir");
    let global_data_dir = proj_dirs.data_dir().to_path_buf();

    let store = FileStore::new(Some(cwd.join(".padz")), global_data_dir);

    // Note: Project scope might not exist if not in a git repo.
    // And `.padz` inside CWD might not be initialized.
    // The spec says `.padz` directory.
    // We pass the PATHs to the store. The store creates them if needed on write.

    let mut api = PadzApi::new(store);

    let scope = if cli.global {
        Scope::Global
    } else {
        Scope::Project
    };

    // Handle commands
    let result = match cli.command {
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

                match edit_content(&initial, ".txt") {
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
        Some(Commands::View { index }) => {
            let idx = parse_index(&index);
            match api.get_pad(&idx, scope) {
                Ok(dp) => {
                    println!(
                        "{} {}",
                        dp.index.to_string().yellow(),
                        dp.pad.metadata.title.bold()
                    );
                    println!("--------------------------------");
                    println!("{}", dp.pad.content);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(Commands::Delete { index }) => {
            let idx = parse_index(&index);
            match api.delete_pad(&idx, scope) {
                Ok(pad) => {
                    println!("Pad deleted: {}", pad.metadata.title.red());
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(Commands::Pin { index }) => {
            let idx = parse_index(&index);
            match api.pin_pad(&idx, scope) {
                Ok(pad) => {
                    println!("Pad pinned: {}", pad.metadata.title.yellow());
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(Commands::Unpin { index }) => {
            let idx = parse_index(&index);
            match api.unpin_pad(&idx, scope) {
                Ok(pad) => {
                    println!("Pad unpinned: {}", pad.metadata.title);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        Some(Commands::Search { term }) => match api.search_pads(&term, scope) {
            Ok(pads) => {
                print_pads(&pads);
                Ok(())
            }
            Err(e) => Err(e),
        },
        Some(Commands::Init) => {
            println!("Initialized padz store");
            Ok(())
        }
        None => {
            // Default to List
            match api.list_pads(scope) {
                Ok(pads) => {
                    print_pads(&pads);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

fn parse_index(s: &str) -> DisplayIndex {
    // Basic parsing: p1 -> Pinned(1), d1 -> Deleted(1), 1 -> Regular(1)
    if let Some(rest) = s.strip_prefix('p')
        && let Ok(n) = rest.parse() {
            return DisplayIndex::Pinned(n);
        }
    if let Some(rest) = s.strip_prefix('d')
        && let Ok(n) = rest.parse() {
            return DisplayIndex::Deleted(n);
        }
    if let Ok(n) = s.parse() {
        return DisplayIndex::Regular(n);
    }
    // Fallback or panic? For CLI better to error properly, but for now simple panic or default
    eprintln!("Invalid index format: {}", s);
    std::process::exit(1);
}

fn print_pads(pads: &[DisplayPad]) {
    if pads.is_empty() {
        println!("No pads found.");
        return;
    }
    for dp in pads {
        let idx_str = dp.index.to_string();
        let idx_colored = match dp.index {
            DisplayIndex::Pinned(_) => idx_str.yellow(),
            DisplayIndex::Deleted(_) => idx_str.red(),
            DisplayIndex::Regular(_) => idx_str.green(),
        };

        let pin_icon = if dp.pad.metadata.is_pinned {
            "📌 "
        } else {
            ""
        };

        println!("{:<4} {}{}", idx_colored, pin_icon, dp.pad.metadata.title);
    }
}
