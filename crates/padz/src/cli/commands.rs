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
//! 2. **Context Setup**: Initialize app state with API, scope, and configuration
//! 3. **Dispatch**: Route commands to handlers via standout's App
//! 4. **Output Formatting**: Use standout templates for rendering
//! 5. **Error Handling**: Convert errors to user-friendly messages and exit codes

use super::handlers::AppState;
use super::setup::{
    build_command, parse_cli, Cli, Commands, CompletionAction, CompletionShell, ConfigSubcommand,
};
use clapfig::{Clapfig, ConfigAction, SearchMode, SearchPath};
use padzapp::config::PadzConfig;
use padzapp::error::Result;
use padzapp::init::initialize;
use standout::cli::{App, RunResult};
use standout::{embed_styles, embed_templates, OutputMode};
use std::io::IsTerminal;

pub fn run() -> Result<()> {
    // parse_cli() uses standout's App which handles
    // help display (including topics) and errors automatically.
    // It also extracts the output mode from the --output flag.
    let (cli, output_mode) = parse_cli();

    // Handle completion before context init (it doesn't need API)
    if let Some(Commands::Completion { shell, action }) = &cli.command {
        return match action {
            CompletionAction::Install => handle_install(*shell),
            CompletionAction::Print => handle_print(*shell),
        };
    }

    // Handle config via clapfig (needs paths but not full API)
    if let Some(Commands::Config { action }) = &cli.command {
        return handle_config(&cli, action);
    }

    // Initialize app state for handlers
    let app_state = create_app_state(&cli, output_mode)?;

    // Determine effective args: handle naked invocation by injecting synthetic command
    let args: Vec<String> = if cli.command.is_none() {
        // Naked padz: list if interactive, create if piped
        let synthetic_cmd = if !std::io::stdin().is_terminal() {
            "create"
        } else {
            "list"
        };
        vec!["padz".to_string(), synthetic_cmd.to_string()]
    } else {
        std::env::args().collect()
    };

    // Build app with injected state, parse, and dispatch through unified path
    let app = build_dispatch_app(app_state);
    let cmd = build_command();
    let matches = app.parse_from(cmd, args);
    handle_dispatch_result(app.dispatch(matches, output_mode))
}

/// Build the dispatch-ready App with templates, styles, command configuration, and app state
fn build_dispatch_app(app_state: AppState) -> App {
    App::builder()
        .app_state(app_state)
        .templates(embed_templates!("src/cli/templates"))
        .template_ext(".jinja")
        .styles(embed_styles!("src/styles"))
        .default_theme("default")
        .commands(Commands::dispatch_config())
        .expect("Failed to configure commands")
        .build()
        .expect("Failed to build app")
}

/// Handle the result of a dispatch operation
fn handle_dispatch_result(result: RunResult) -> Result<()> {
    match result {
        RunResult::Handled(output) => {
            if output.starts_with("Error:") {
                return Err(padzapp::error::PadzError::Api(
                    output.trim_start_matches("Error: ").to_string(),
                ));
            }
            print!("{}", output);
        }
        RunResult::Binary(data, filename) => {
            std::fs::write(&filename, &data)
                .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?;
            println!("Exported to {}", filename);
        }
        RunResult::Silent => {}
        RunResult::NoMatch(_) => {
            eprintln!("Error: Unknown command");
        }
    }
    Ok(())
}

/// Create app state containing API, scope, and configuration for handlers
fn create_app_state(cli: &Cli, output_mode: OutputMode) -> Result<AppState> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let data_override = cli.data.as_ref().map(std::path::PathBuf::from);

    // `padz init` (plain, non-global) is a creation operation: "create a store HERE".
    // It should use cwd directly, not walk up to find an existing store.
    // All other commands use find_padz_root() for discovery, which is correct.
    let is_plain_init = matches!(
        cli.command,
        Some(Commands::Init {
            link: None,
            unlink: false
        })
    );
    let data_override = if is_plain_init && data_override.is_none() && !cli.global {
        Some(cwd.clone())
    } else {
        data_override
    };

    // Commands that create new pads opt into auto-init: if no `.padz` is found
    // upward, a fresh store is materialized at the enclosing git root (if any)
    // so the new pad is project-scoped rather than silently dropped into global.
    // Every other command operates on existing pads and reads fall back to global
    // cleanly; they pass `false`.
    let auto_init_for_write = matches!(
        cli.command,
        Some(Commands::Create { .. }) | Some(Commands::Import { .. })
    );

    // Compute the local .padz dir BEFORE link resolution (used by link/unlink commands)
    let local_padz_dir = match &data_override {
        Some(path) => {
            if path.file_name().is_some_and(|name| name == ".padz") {
                path.clone()
            } else {
                path.join(".padz")
            }
        }
        None => padzapp::init::find_padz_root(&cwd)
            .map(|root| root.join(".padz"))
            .unwrap_or_else(|| cwd.join(".padz")),
    };

    let padz_ctx = initialize(&cwd, cli.global, data_override, auto_init_for_write)?;

    Ok(AppState::new(
        padz_ctx.api,
        padz_ctx.scope,
        padz_ctx.config.import_extensions(),
        output_mode,
        padz_ctx.config.mode,
        local_padz_dir,
    ))
}

fn handle_config(cli: &Cli, subcommand: &Option<ConfigSubcommand>) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let data_override = cli.data.as_ref().map(std::path::PathBuf::from);

    // Resolve paths (lightweight version of initialize — just need dirs, not full API)
    let project_padz_dir = match data_override {
        Some(path) => {
            if path.file_name().is_some_and(|name| name == ".padz") {
                path
            } else {
                path.join(".padz")
            }
        }
        None => padzapp::init::find_padz_root(&cwd)
            .map(|root| root.join(".padz"))
            .unwrap_or_else(|| cwd.join(".padz")),
    };

    let global_data_dir = std::env::var("PADZ_GLOBAL_DATA")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            let proj_dirs = directories::ProjectDirs::from("com", "padz", "padz")
                .expect("Could not determine config dir");
            proj_dirs.data_dir().to_path_buf()
        });

    // Map -g flag to clapfig scope: None defaults to "local" (first registered)
    let scope: Option<String> = if cli.global {
        Some("global".into())
    } else {
        None
    };

    // Convert our ConfigSubcommand to clapfig's ConfigAction
    let action = match subcommand {
        None => ConfigAction::List {
            scope: scope.clone(),
        },
        Some(ConfigSubcommand::List) => ConfigAction::List {
            scope: scope.clone(),
        },
        Some(ConfigSubcommand::Gen { file }) => ConfigAction::Gen {
            output: file.clone(),
        },
        Some(ConfigSubcommand::Get { key }) => ConfigAction::Get {
            key: key.clone(),
            scope: scope.clone(),
        },
        Some(ConfigSubcommand::Set { key, value }) => ConfigAction::Set {
            key: key.clone(),
            value: value.clone(),
            scope,
        },
    };

    Clapfig::builder::<PadzConfig>()
        .app_name("padz")
        .file_name("padz.toml")
        .search_paths(vec![
            SearchPath::Path(global_data_dir.clone()),
            SearchPath::Path(project_padz_dir.clone()),
        ])
        .search_mode(SearchMode::Merge)
        .strict(false)
        .persist_scope("local", SearchPath::Path(project_padz_dir))
        .persist_scope("global", SearchPath::Path(global_data_dir))
        .handle_and_print(&action)
        .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?;

    Ok(())
}

fn handle_print(shell_override: Option<CompletionShell>) -> Result<()> {
    let shell = resolve_shell(shell_override)?;
    print!("{}", completion_script(shell)?);
    Ok(())
}

fn handle_install(shell_override: Option<CompletionShell>) -> Result<()> {
    let shell = resolve_shell(shell_override)?;
    let script = completion_script(shell)?;

    let path = install_path(shell)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, script)?;

    println!("Completions installed to {}", path.display());
    for line in post_install_hint(shell, &path) {
        println!("{}", line);
    }

    Ok(())
}

fn resolve_shell(shell_override: Option<CompletionShell>) -> Result<CompletionShell> {
    shell_override.or_else(detect_shell).ok_or_else(|| {
        padzapp::error::PadzError::Api(
            "Could not detect shell from $SHELL. Use --shell bash|zsh|fish".into(),
        )
    })
}

fn detect_shell() -> Option<CompletionShell> {
    let shell = std::env::var("SHELL").ok()?;
    let name = std::path::Path::new(&shell).file_name()?.to_str()?;
    match name {
        "bash" => Some(CompletionShell::Bash),
        "zsh" => Some(CompletionShell::Zsh),
        "fish" => Some(CompletionShell::Fish),
        _ => None,
    }
}

/// Generate the completion script by re-invoking self with COMPLETE=<shell>.
///
/// clap_complete's `CompleteEnv` in main.rs intercepts this and prints the
/// dynamic-completion registration script for the chosen shell. Re-using that
/// path ensures the installed script always matches the binary's wire protocol.
fn completion_script(shell: CompletionShell) -> Result<String> {
    let exe = std::env::current_exe().map_err(|e| {
        padzapp::error::PadzError::Api(format!("cannot locate current executable: {e}"))
    })?;
    let output = std::process::Command::new(&exe)
        .env("COMPLETE", shell.as_complete_env())
        .output()
        .map_err(|e| {
            padzapp::error::PadzError::Api(format!("failed to invoke {}: {e}", exe.display()))
        })?;
    if !output.status.success() {
        return Err(padzapp::error::PadzError::Api(format!(
            "completion generator exited with status {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    String::from_utf8(output.stdout).map_err(|e| {
        padzapp::error::PadzError::Api(format!("completion script not valid UTF-8: {e}"))
    })
}

fn xdg_data_home() -> Result<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("XDG_DATA_HOME") {
        if !dir.is_empty() {
            return Ok(std::path::PathBuf::from(dir));
        }
    }
    let home = std::env::var("HOME").map_err(|_| {
        padzapp::error::PadzError::Api("$HOME not set; cannot determine install path".into())
    })?;
    Ok(std::path::PathBuf::from(home).join(".local/share"))
}

fn xdg_config_home() -> Result<std::path::PathBuf> {
    if let Ok(dir) = std::env::var("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Ok(std::path::PathBuf::from(dir));
        }
    }
    let home = std::env::var("HOME").map_err(|_| {
        padzapp::error::PadzError::Api("$HOME not set; cannot determine install path".into())
    })?;
    Ok(std::path::PathBuf::from(home).join(".config"))
}

fn install_path(shell: CompletionShell) -> Result<std::path::PathBuf> {
    Ok(match shell {
        CompletionShell::Bash => xdg_data_home()?.join("bash-completion/completions/padz"),
        CompletionShell::Zsh => xdg_data_home()?.join("zsh/site-functions/_padz"),
        CompletionShell::Fish => xdg_config_home()?.join("fish/completions/padz.fish"),
    })
}

/// Post-install messaging — tells the user exactly what (if anything) they
/// still need to do for completions to activate.
fn post_install_hint(shell: CompletionShell, path: &std::path::Path) -> Vec<String> {
    match shell {
        CompletionShell::Fish => {
            vec!["Completions will activate in new fish shells (no further action needed).".into()]
        }
        CompletionShell::Zsh => {
            let dir = path
                .parent()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            if zsh_dir_on_fpath(&dir) {
                vec!["Completions will activate in new zsh shells.".into()]
            } else {
                vec![
                    "To activate, add this line to ~/.zshrc (one-time setup):".into(),
                    format!("  fpath=({} $fpath)", dir),
                    "Then restart your shell (or run: autoload -Uz compinit && compinit).".into(),
                ]
            }
        }
        CompletionShell::Bash => {
            let mut hints = vec![
                "Completions will activate in new bash shells.".into(),
                format!("To use now in the current shell: source {}", path.display()),
            ];
            if cfg!(target_os = "macos") && !macos_bash_completion_available() {
                hints.push(String::new());
                hints.push(
                    "Note: on macOS, bash completions only load if the bash-completion".into(),
                );
                hints.push(
                    "package is installed. Install it with: brew install bash-completion@2".into(),
                );
                hints.push("and follow the post-install instructions brew prints.".into());
            }
            hints
        }
    }
}

/// Probe zsh to check if `dir` is already on `$fpath`. Non-interactive zsh
/// skips `.zshrc`, so we run an interactive instance with a short timeout to
/// get the user's real fpath. Returns false on any failure (the fallback path
/// just asks the user to add the fpath line manually, which is always safe).
fn zsh_dir_on_fpath(dir: &str) -> bool {
    use std::process::Command;
    let Ok(output) = Command::new("zsh")
        .arg("-ic")
        .arg("print -rl -- $fpath")
        .output()
    else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let canon = std::fs::canonicalize(dir).ok();
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        let line = line.trim();
        if line == dir {
            return true;
        }
        match (canon.as_ref(), std::fs::canonicalize(line).ok()) {
            (Some(a), Some(b)) => *a == b,
            _ => false,
        }
    })
}

/// Check whether /etc/bash_completion or a brew-installed bash-completion is
/// present on macOS. Used only for advisory post-install messaging.
fn macos_bash_completion_available() -> bool {
    for path in [
        "/opt/homebrew/etc/profile.d/bash_completion.sh",
        "/usr/local/etc/profile.d/bash_completion.sh",
        "/opt/homebrew/etc/bash_completion",
        "/usr/local/etc/bash_completion",
        "/etc/bash_completion",
    ] {
        if std::path::Path::new(path).exists() {
            return true;
        }
    }
    false
}
