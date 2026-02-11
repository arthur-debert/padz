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
use super::setup::{build_command, parse_cli, Cli, Commands, CompletionAction, CompletionShell};
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

    let padz_ctx = initialize(&cwd, cli.global, data_override);
    let scope = if cli.global {
        padzapp::model::Scope::Global
    } else {
        padzapp::model::Scope::Project
    };

    Ok(AppState::new(
        padz_ctx.api,
        scope,
        padz_ctx.config.import_extensions.clone(),
        output_mode,
    ))
}

fn handle_print(shell_override: Option<CompletionShell>) -> Result<()> {
    let shell = resolve_shell(shell_override)?;
    print!("{}", completion_script(shell));
    Ok(())
}

fn handle_install(shell_override: Option<CompletionShell>) -> Result<()> {
    let shell = resolve_shell(shell_override)?;
    let script = completion_script(shell);

    let path = install_path(shell)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, script)?;

    println!("Completions installed to {}", path.display());
    match shell {
        CompletionShell::Bash => {
            println!("Restart your shell or run: source {}", path.display());
        }
        CompletionShell::Zsh => {
            if !zshrc_has_zfunc() {
                println!("Add the following to your ~/.zshrc:");
                println!("  fpath=(~/.zfunc $fpath)");
                println!("  autoload -Uz compinit && compinit");
            } else {
                println!("Restart your shell to activate.");
            }
        }
    }

    Ok(())
}

fn resolve_shell(shell_override: Option<CompletionShell>) -> Result<CompletionShell> {
    shell_override.or_else(detect_shell).ok_or_else(|| {
        padzapp::error::PadzError::Api(
            "Could not detect shell from $SHELL. Use --shell bash or --shell zsh".into(),
        )
    })
}

fn detect_shell() -> Option<CompletionShell> {
    let shell = std::env::var("SHELL").ok()?;
    let name = std::path::Path::new(&shell).file_name()?.to_str()?;
    match name {
        "bash" => Some(CompletionShell::Bash),
        "zsh" => Some(CompletionShell::Zsh),
        _ => None,
    }
}

fn completion_script(shell: CompletionShell) -> &'static str {
    match shell {
        CompletionShell::Bash => {
            r#"# padz bash completions
_padz() {
    local IFS=$'\n'
    local cur="${COMP_WORDS[COMP_CWORD]}"
    local candidates
    candidates=$(COMP_WORDS="${COMP_WORDS[*]}" COMP_CWORD="$COMP_CWORD" _CLAP_COMPLETE=bash padz 2>/dev/null)
    if [[ $? -eq 0 ]]; then
        COMPREPLY=($(compgen -W "$candidates" -- "$cur"))
    fi
}
complete -F _padz padz
"#
        }
        CompletionShell::Zsh => {
            r#"#compdef padz

_padz() {
    local IFS=$'\n'
    local candidates
    candidates=("${(@f)$(COMP_WORDS="${words[*]}" COMP_CWORD=$((CURRENT - 1)) _CLAP_COMPLETE=zsh padz 2>/dev/null)}")
    if [[ $? -eq 0 ]]; then
        _describe 'command' candidates
    fi
}

compdef _padz padz
"#
        }
    }
}

fn install_path(shell: CompletionShell) -> Result<std::path::PathBuf> {
    let home = std::env::var("HOME").map_err(|_| {
        padzapp::error::PadzError::Api("$HOME not set; cannot determine install path".into())
    })?;

    Ok(match shell {
        CompletionShell::Bash => {
            let data_dir =
                std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| format!("{}/.local/share", home));
            std::path::PathBuf::from(data_dir).join("bash-completion/completions/padz")
        }
        CompletionShell::Zsh => std::path::PathBuf::from(&home).join(".zfunc/_padz"),
    })
}

/// Checks if ~/.zshrc contains a reference to .zfunc in fpath.
fn zshrc_has_zfunc() -> bool {
    let Ok(home) = std::env::var("HOME") else {
        return false;
    };
    let zshrc = std::path::PathBuf::from(home).join(".zshrc");
    let Ok(content) = std::fs::read_to_string(zshrc) else {
        return false;
    };
    content.lines().any(|line| {
        let trimmed = line.trim();
        !trimmed.starts_with('#') && trimmed.contains("fpath") && trimmed.contains(".zfunc")
    })
}
