//! Shared fixtures for the in-process test groups (typed-handler + harness).
//!
//! # The seam this module exists to hold
//!
//! Both in-process groups need the same thing: a real padz store in a tempdir,
//! and the CLI wired to it. The wiring is [`cli::commands::build_app_state`],
//! which takes the environment and cwd as explicit values rather than reading
//! `$PADZ_GLOBAL_DATA` and `current_dir` off the process. That is what lets a
//! fixture point padz at a tempdir *in Rust* — no env mutation, no cwd juggling,
//! and therefore no reason for these tests to fight each other.
//!
//! The `TestHarness` group still mutates process-global state for the seams the
//! harness owns (terminal detectors, default stdin/clipboard readers), which is
//! why those tests are `#[serial]`. This fixture is not the reason they are
//! serial, and using it does not make a test need `#[serial]` by itself.

#![allow(dead_code)] // Each test binary uses its own subset of these helpers.

use clap::Parser;
use padz::cli::commands::{build_app_state, build_dispatch_app};
use padz::cli::handlers::AppState;
use padz::cli::input::RequestContent;
use padz::cli::setup::{build_command, Cli};
use padzapp::init::{create_bucket_layout, PadzEnv};
use standout::cli::App;
use standout::input::{InputSourceKind, Inputs, ResolvedInput};
use standout_dispatch::{CommandContext, Extensions};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tempfile::TempDir;

/// A padz project store in a tempdir, plus the environment pointing at it.
///
/// Drop order matters: the `TempDir` lives as long as the fixture, so anything
/// borrowing paths from it stays valid for the test.
pub struct Fixture {
    temp: TempDir,
    project: PathBuf,
    env: PadzEnv,
}

impl Fixture {
    /// Creates an initialized project store (`<temp>/project/.padz`) and an
    /// isolated global store (`<temp>/global`).
    ///
    /// The global dir is isolated even though every helper here selects the
    /// project scope: an accidental global fallback should land in the tempdir,
    /// not in the developer's real `~` — a test that silently wrote to the real
    /// global store would be both a false pass and a mess.
    pub fn new() -> Self {
        let temp = tempfile::tempdir().expect("failed to create tempdir");
        let project = temp.path().join("project");
        let global = temp.path().join("global");
        std::fs::create_dir_all(&project).expect("failed to create project dir");
        std::fs::create_dir_all(&global).expect("failed to create global dir");
        // What `padz init` materializes: active/, archived/, deleted/.
        create_bucket_layout(&project.join(".padz")).expect("failed to init store layout");

        let env = PadzEnv {
            global_data_dir: global,
            home_dir: Some(temp.path().to_path_buf()),
        };
        Self { temp, project, env }
    }

    /// The project root (the directory *containing* `.padz`).
    pub fn project(&self) -> &Path {
        &self.project
    }

    /// Switches this fixture's store to todos mode.
    ///
    /// Writes the config file rather than shelling out to `padz config set`,
    /// because `config` is handled by `cli::run` *before* dispatch (clapfig owns
    /// it) and so is not reachable through the app under test. The mode reaches
    /// the CLI as `AppState.mode`, read from this file at state-build time, so
    /// writing it is the same input by a shorter path.
    ///
    /// Must be called before [`app_state`](Self::app_state): the config is read
    /// once, when state is built.
    pub fn todos_mode(&self) -> &Self {
        std::fs::write(
            self.project.join(".padz").join("padz.toml"),
            "mode = \"todos\"\n",
        )
        .expect("failed to write padz.toml");
        self
    }

    /// The tempdir root, for tests that need a path outside the project.
    pub fn root(&self) -> &Path {
        self.temp.path()
    }

    /// The `--data` value that binds an invocation to this fixture's store.
    fn data_arg(&self) -> &str {
        self.project.to_str().expect("tempdir path is not UTF-8")
    }

    /// Argv for this fixture's store: `["padz", "--data", <project>, ...args]`.
    ///
    /// Every in-process invocation goes through here, so no test can forget the
    /// `--data` binding and silently operate on the ambient store.
    pub fn argv<'a>(&'a self, args: &[&'a str]) -> Vec<&'a str> {
        let mut argv = vec!["padz", "--data", self.data_arg()];
        argv.extend_from_slice(args);
        argv
    }

    /// App state bound to this fixture's store, for an ordinary read/modify command.
    ///
    /// Spelled `list` because the command only reaches `build_app_state` as an
    /// auto-init question — "should a missing store be created for this write?" —
    /// and `list` answers it the same way every non-create command does: no. Tests
    /// whose command *does* change that answer (`create`, `import`, `init`) should
    /// say so via [`app_state_for`](Self::app_state_for).
    pub fn app_state(&self) -> AppState {
        self.app_state_for(&["list"])
    }

    /// App state bound to this fixture's store, as the given argv would produce.
    ///
    /// `args` must parse as a real padz invocation, so a command with required
    /// positionals needs them here (`&["view", "1"]`). Only `create`, `import`, and
    /// plain `init` actually read differently; see [`app_state`](Self::app_state).
    pub fn app_state_for(&self, args: &[&str]) -> AppState {
        let cli = Cli::try_parse_from(self.argv(args)).unwrap_or_else(|e| {
            panic!("fixture argv {args:?} is not a valid padz invocation: {e}")
        });
        build_app_state(&cli, &self.env, &self.project).expect("failed to build app state")
    }

    /// The real dispatch app (templates, styles, dispatch table) bound to this
    /// fixture's store, paired with the clap command — the two arguments
    /// `TestHarness::run` wants.
    ///
    /// `args` is the invocation whose *app state* is being built, not the argv the
    /// harness will run; pass the latter to `TestHarness::run` via [`argv`](Self::argv).
    pub fn app(&self, args: &[&str]) -> (App, clap::Command) {
        (
            build_dispatch_app(self.app_state_for(args)),
            build_command(),
        )
    }

    /// The dispatch app for an ordinary read/modify command. See [`app_state`](Self::app_state).
    pub fn read_app(&self) -> (App, clap::Command) {
        self.app(&["list"])
    }

    /// A `CommandContext` carrying this fixture's app state, for calling typed
    /// handler functions directly.
    pub fn ctx(&self) -> CommandContext {
        ctx_with_state(self.app_state())
    }

    /// Creates a pad directly through the API, for arranging test state.
    ///
    /// Arrangement goes through the API rather than a handler so a test's *setup*
    /// can't fail for the reason the test is checking.
    pub fn seed_pad(&self, state: &AppState, title: &str, body: &str) {
        state
            .with_api(|api| api.create_pad(state.scope, title.to_string(), body.to_string(), None))
            .unwrap_or_else(|e| panic!("failed to seed pad {title:?}: {e}"));
    }

    /// Creates a child pad under a canonical display selector.
    pub fn seed_child(&self, state: &AppState, parent: &str, title: &str, body: &str) {
        state
            .with_api(|api| {
                api.create_pad(
                    state.scope,
                    title.to_string(),
                    body.to_string(),
                    Some(parent),
                )
            })
            .unwrap_or_else(|e| panic!("failed to seed child pad {title:?}: {e}"));
    }
}

impl Default for Fixture {
    fn default() -> Self {
        Self::new()
    }
}

/// Wraps an `AppState` in a `CommandContext`, the way dispatch would.
pub fn ctx_with_state(state: AppState) -> CommandContext {
    let mut ext = Extensions::new();
    ext.insert(state);
    CommandContext::new(Vec::new(), Rc::new(ext))
}

/// A `CommandContext` whose input bag already holds `content` under `name`,
/// standing in for the input chain that would have resolved it pre-dispatch.
///
/// This is how a typed-handler test drives `create`/`edit` without stdin: the
/// chain's *own* precedence is proven at the harness seam, where real piped stdin
/// exists. Here the handler is asked only what it does with an already-resolved
/// decision — which is the whole of what it is allowed to know.
pub fn ctx_with_input(
    state: AppState,
    name: &'static str,
    content: RequestContent,
) -> CommandContext {
    let mut ctx = ctx_with_state(state);
    let mut inputs = Inputs::new();
    inputs.insert(
        name,
        ResolvedInput {
            value: content,
            source: InputSourceKind::Arg,
        },
    );
    ctx.extensions.insert(inputs);
    ctx
}
