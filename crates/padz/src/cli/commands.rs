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
use super::setup::{build_command, parse_cli, Cli, Commands, CompletionShell};
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

    // Handle completions before context init (they don't need API)
    if let Some(Commands::Completions { shell }) = &cli.command {
        return handle_completions(*shell);
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

fn handle_completions(shell: CompletionShell) -> Result<()> {
    // The user adds this to their shell rc file: eval "$(padz completions bash)"
    match shell {
        CompletionShell::Bash => {
            let script = r#"# padz bash completions
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
"#;
            print!("{}", script);
        }
        CompletionShell::Zsh => {
            let script = r#"#compdef padz

_padz() {
    local IFS=$'\n'
    local candidates
    candidates=("${(@f)$(COMP_WORDS="${words[*]}" COMP_CWORD=$((CURRENT - 1)) _CLAP_COMPLETE=zsh padz 2>/dev/null)}")
    if [[ $? -eq 0 ]]; then
        _describe 'command' candidates
    fi
}

compdef _padz padz
"#;
            print!("{}", script);
        }
    }

    Ok(())
}
