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

/// `/qc[=type]` quick compare of two files → exit code 1/2/11/12/13 (`TQuickCompareFilesJob`).
///
/// * `size` / `crc` / `binary`: byte-level, the folder compare's reader → `1` same, `11` different.
/// * rules-based (`None`, the reference default): byte-identical or no differences under the
///   format's default rules → `2`; only unimportant differences → `12`; any important
///   difference → `13`. Input the text engine rejects (binary files) → `11`, as the byte
///   comparison has already shown they differ.
///
/// `Err` carries a message for I/O and engine failures (the caller exits with `100`).
pub fn quick_compare(
    vfs: &VfsManager,
    left: &bc_vfs::VfsPath,
    right: &bc_vfs::VfsPath,
    mode: Option<bc_engine_dir::criteria::ContentCompare>,
    formats: &FormatRegistry,
) -> Result<ExitCode, String> {
    use bc_engine_dir::{content::compare_readers, CompareStatus, ContentCompare};
    use bc_formats::FormatKind;
    use std::io::Read;

    let cancel = bc_core::cancel::NeverCancel;
    let open = |path: &bc_vfs::VfsPath| {
        vfs.resolve(path)
            .and_then(|fs| fs.open_read(path))
            .map_err(|e| format!("cannot read {path}: {e}"))
    };
    let (mut l, mut r) = (open(left)?, open(right)?);
    let mode = mode.unwrap_or(ContentCompare::RulesBased);
    if mode != ContentCompare::RulesBased {
        let (status, _) =
            compare_readers(&mut l, &mut r, mode, &cancel).map_err(|e| e.to_string())?;
        return Ok(if status == CompareStatus::Same {
            ExitCode::BinarySame
        } else {
            ExitCode::BinaryDifferent
        });
    }
    let read = |reader: &mut dyn Read, path: &bc_vfs::VfsPath| {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map(|_| bytes)
            .map_err(|e| format!("cannot read {path}: {e}"))
    };
    let (l, r) = (read(&mut l, left)?, read(&mut r, right)?);
    if l == r {
        return Ok(ExitCode::RulesBasedSame);
    }
    let format = |path: &bc_vfs::VfsPath| {
        path.file_name()
            .and_then(|name| formats.resolve(std::path::Path::new(name), Some(FormatKind::Text)))
            .cloned()
    };
    let (Ok(l), Ok(r)) = (
        bc_engine_text::TextContent::from_bytes(l, format(left)),
        bc_engine_text::TextContent::from_bytes(r, format(right)),
    ) else {
        return Ok(ExitCode::BinaryDifferent);
    };
    let mut map = bc_engine_text::TextMap::new(l, r, None, bc_engine_text::TextRules::default());
    let output = bc_engine_text::jobs::Align2Job::from_map(&map)
        .execute(&cancel)
        .map_err(|e| e.to_string())?;
    map.set_alignment(output).map_err(|e| e.to_string())?;
    let counts = map.statistics();
    Ok(if counts.important > 0 {
        ExitCode::RulesBasedDifferent
    } else if counts.unimportant > 0 {
        ExitCode::Similar
    } else {
        ExitCode::RulesBasedSame
    })
}

/// [`quick_compare`] on two local files with the built-in plus user file formats (an
/// unreadable user format folder falls back to the built-ins, as the folder compare does).
pub fn quick_compare_files(
    left: &std::path::Path,
    right: &std::path::Path,
    mode: Option<bc_engine_dir::criteria::ContentCompare>,
) -> Result<ExitCode, String> {
    for path in [left, right] {
        if path.is_dir() {
            return Err(format!(
                "/qc compares two files; {} is a folder",
                path.display()
            ));
        }
        if !path.is_file() {
            return Err(format!("/qc: {} does not exist", path.display()));
        }
    }
    let formats = bc_formats::registry::user_formats_dir()
        .ok()
        .and_then(|dir| FormatRegistry::load_user_dir(&dir).ok())
        .unwrap_or_else(FormatRegistry::with_builtins);
    let vfs = VfsManager::local_only();
    quick_compare(
        &vfs,
        &bc_vfs::VfsPath::local(left),
        &bc_vfs::VfsPath::local(right),
        mode,
        &formats,
    )
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
