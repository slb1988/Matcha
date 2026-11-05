//! Launcher logic shared by `matcha-comp` (GUI launcher, returns at once) and
//! `matcha-compc` (console launcher, waits for the comparison and returns its exit code).
//!
//! Reference behaviour (command_line_reference "program trio"):
//! * locate the main executable next to the launcher (exit 103 when missing);
//! * forward all arguments unchanged, plus a hidden handle so the main instance can
//!   report completion back (exit 102 when waiting is impossible);
//! * `matcha-compc` must have a console; it prints `/?` usage itself.
#![allow(unused_variables)] // scaffold: bodies are `todo!()`; remove once implemented
#![forbid(unsafe_code)]

use bc_script::ExitCode;
use std::path::PathBuf;

/// Which launcher flavour is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// `matcha-comp`: start the GUI (or hand off to the running instance) and return 0.
    ReturnImmediately,
    /// `matcha-compc`: wait for the comparison/script to finish and return its exit code.
    WaitForExitCode,
}

/// Name of the main executable looked up next to the launcher.
pub const MAIN_EXECUTABLE: &str = if cfg!(windows) { "matcha.exe" } else { "matcha" };

/// Find the main executable (same directory as the launcher, then `PATH`).
pub fn locate_main_executable() -> Result<PathBuf, ExitCode> {
    todo!("bcomp: resolve MAIN_EXECUTABLE next to current_exe(), else PATH; Err(LauncherCannotFindMain)")
}

/// Run the launcher with the process arguments (without argv[0]).
pub fn launch(mode: LaunchMode, args: Vec<String>) -> ExitCode {
    todo!("bcomp: spawn main executable, optionally wait on completion handle (LauncherCannotWait on failure)")
}
