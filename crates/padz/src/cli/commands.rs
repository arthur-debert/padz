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
//! 2. **Context Setup**: Initialize handler context with API, scope, and configuration
//! 3. **Dispatch**: Route commands to handlers via standout's App
//! 4. **Output Formatting**: Use standout templates for rendering
//! 5. **Error Handling**: Convert errors to user-friendly messages and exit codes

use super::handlers::{self, HandlerContext};
use super::setup::{build_command, parse_cli, Cli, Commands, CompletionShell};
use padzapp::error::Result;
use padzapp::init::initialize;
use standout::cli::{App, RunResult, ThreadSafe};
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

    // Initialize handler context (thread-local storage for handlers)
    let handler_ctx = init_context(&cli, output_mode)?;
    handlers::init_context(handler_ctx);

    // Handle naked invocation (no command specified)
    if cli.command.is_none() {
        // Naked padz: list if interactive, create if piped
        if !std::io::stdin().is_terminal() {
            // Piped input -> create
            return run_dispatch("create", output_mode);
        } else {
            // Interactive -> list
            return run_dispatch("list", output_mode);
        }
    }

    // Build and run the app with dispatch configuration
    let app = App::<ThreadSafe>::builder()
        .templates(embed_templates!("src/cli/templates"))
        .styles(embed_styles!("src/styles"))
        .default_theme("default")
        .commands(Commands::dispatch_config())
        .expect("Failed to configure commands")
        .build()
        .expect("Failed to build app");

    // Parse and dispatch
    let cmd = build_command();
    let matches = app.parse_from(cmd, std::env::args());

    match app.dispatch(matches, output_mode) {
        RunResult::Handled(output) => {
            print!("{}", output);
        }
        RunResult::Binary(data, filename) => {
            // Write binary output to file
            std::fs::write(&filename, &data)
                .map_err(|e| padzapp::error::PadzError::Api(e.to_string()))?;
            println!("Exported to {}", filename);
        }
        RunResult::Silent => {}
        RunResult::NoMatch(_) => {
            // Should not happen with our setup
            eprintln!("Error: Unknown command");
        }
    }

    Ok(())
}

/// Run dispatch for a specific command (used for naked invocation)
fn run_dispatch(command: &str, output_mode: OutputMode) -> Result<()> {
    let app = App::<ThreadSafe>::builder()
        .templates(embed_templates!("src/cli/templates"))
        .styles(embed_styles!("src/styles"))
        .default_theme("default")
        .commands(Commands::dispatch_config())
        .expect("Failed to configure commands")
        .build()
        .expect("Failed to build app");

    // Build args with the command inserted
    let args = vec!["padz".to_string(), command.to_string()];
    let cmd = build_command();
    let matches = app.parse_from(cmd, args);

    match app.dispatch(matches, output_mode) {
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

fn init_context(cli: &Cli, output_mode: OutputMode) -> Result<HandlerContext> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let data_override = cli.data.as_ref().map(std::path::PathBuf::from);

    let padz_ctx = initialize(&cwd, cli.global, data_override);
    let scope = if cli.global {
        padzapp::model::Scope::Global
    } else {
        padzapp::model::Scope::Project
    };

    Ok(HandlerContext {
        api: padz_ctx.api,
        scope,
        import_extensions: padz_ctx.config.import_extensions.clone(),
        output_mode,
    })
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
