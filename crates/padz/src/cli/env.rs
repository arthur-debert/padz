//! The composition root for environment- and platform-derived settings.
//!
//! `padzapp` takes its paths as explicit inputs; deciding what those inputs
//! *are* on this machine is the application's job, and this module is the one
//! place that does it. Everything below reads process state (`$PADZ_GLOBAL_DATA`)
//! or asks the OS where user directories live — which is exactly why it lives
//! in the CLI and not in the library.

use padzapp::init::PadzEnv;
use std::path::PathBuf;

/// Resolves the environment `padzapp::init::initialize` needs.
///
/// - **global data dir**: `$PADZ_GLOBAL_DATA` when set (the escape hatch tests
///   and scripts use to isolate global state), else the OS-appropriate data
///   directory.
/// - **home dir**: the boundary the upward `.padz`/`.git` walks stop at.
///   `None` when it can't be determined, which lets the walk run to the
///   filesystem root rather than failing the command.
pub fn resolve() -> PadzEnv {
    PadzEnv {
        global_data_dir: global_data_dir(),
        home_dir: home_dir(),
    }
}

/// Where global-scope pads live on this machine.
///
/// # Panics
///
/// Panics when neither `$PADZ_GLOBAL_DATA` is set nor an OS data directory can
/// be determined — padz has nowhere to keep global pads and cannot sensibly
/// continue. This preserves the pre-existing behavior of `initialize`.
pub fn global_data_dir() -> PathBuf {
    std::env::var("PADZ_GLOBAL_DATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            let proj_dirs = directories::ProjectDirs::from("com", "padz", "padz")
                .expect("Could not determine config dir");
            proj_dirs.data_dir().to_path_buf()
        })
}

/// The user's home directory, if it can be determined.
fn home_dir() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|bd| bd.home_dir().to_path_buf())
}
