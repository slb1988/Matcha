//! Script interpreter – runs a parsed script sequentially against the engines
//! (`TDIRSCRIPTDIALOG` / `TScriptDialogOpeningEventArgs` backend). Also the executor for
//! `/qc` quick compares and `/automerge`, which share the exit-code logic.

use crate::command::{Command, LogLevel, ScriptOption};
use crate::exit_code::ExitCode;
use crate::parser::ScriptLine;
use bc_engine_dir::DirTree;
use bc_formats::registry::FormatRegistry;
use bc_jobs::JobManager;
use bc_session::SessionManager;
use bc_vfs::VfsManager;
use std::path::PathBuf;
use std::sync::Arc;

/// A loaded script ready to run.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    pub path: Option<PathBuf>,
    pub lines: Vec<ScriptLine>,
}

impl Script {
    /// Read and parse a script file (`@file`). Errors map to exit 105/106.
    pub fn load(path: &std::path::Path, args: &[String]) -> Result<Script, ExitCode> {
        todo!("bc-script::runner: read file, parse_script, map errors to exit codes")
    }
}

/// Runtime options from the command line that influence script execution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunnerOptions {
    /// `/silent`: questions are logged instead of asked.
    pub silent: bool,
    /// `/closescript`: close the script window when done (GUI host).
    pub close_when_done: bool,
}

/// Mutable interpreter state.
pub struct ScriptRunner {
    pub options: RunnerOptions,
    pub vfs: Arc<VfsManager>,
    pub jobs: Arc<JobManager>,
    pub formats: Arc<FormatRegistry>,
    pub sessions: SessionManager,
    /// Current folder comparison (after `load`).
    pub tree: Option<DirTree>,
    /// Current selection (relative paths).
    pub selection: Vec<String>,
    pub log_level: LogLevel,
    pub log_file: Option<PathBuf>,
    pub stop_on_error: bool,
    pub confirm: Option<ScriptOption>,
}

impl ScriptRunner {
    pub fn new(
        options: RunnerOptions,
        vfs: Arc<VfsManager>,
        jobs: Arc<JobManager>,
        formats: Arc<FormatRegistry>,
        sessions: SessionManager,
    ) -> Self {
        Self {
            options,
            vfs,
            jobs,
            formats,
            sessions,
            tree: None,
            selection: Vec::new(),
            log_level: LogLevel::None,
            log_file: None,
            stop_on_error: false,
            confirm: None,
        }
    }

    /// Execute every line; returns the process exit code.
    pub fn run(&mut self, script: &Script) -> ExitCode {
        todo!("bc-script::runner: sequential execution with logging")
    }

    /// Execute one command.
    pub fn execute(&mut self, command: &Command) -> Result<(), ExitCode> {
        todo!("bc-script::runner: dispatch to engines")
    }

    /// Append to the script log honouring `log` level/file.
    pub fn log(&mut self, level: LogLevel, message: &str) {
        todo!("bc-script::runner: logging")
    }
}

/// `/qc[=type]` quick compare of two files → exit code 1/2/11/12/13.
pub fn quick_compare(
    vfs: &VfsManager,
    left: &bc_vfs::VfsPath,
    right: &bc_vfs::VfsPath,
    mode: Option<bc_engine_dir::criteria::ContentCompare>,
    formats: &FormatRegistry,
) -> ExitCode {
    todo!("bc-script::runner: quick compare → exit code")
}

/// `/automerge` → exit code 0/14/101 per merge outcome.
pub fn auto_merge_exit_code(outcome: &bc_engine_text::merge::AutoMergeOutcome) -> ExitCode {
    if outcome.conflicts.is_empty() {
        ExitCode::Success
    } else if outcome.withheld {
        ExitCode::ConflictsMergeNotWritten
    } else {
        ExitCode::ConflictsDetected
    }
}
