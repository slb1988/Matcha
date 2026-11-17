//! Script interpreter – runs a parsed script sequentially against the engines
//! (`TDIRSCRIPTDIALOG` / `TScriptDialogOpeningEventArgs` backend). Also the executor for
//! `/qc` quick compares and `/automerge`, which share the exit-code logic.

use crate::command::{Command, LogLevel, ScriptOption};
use crate::exit_code::ExitCode;
use crate::parser::ScriptLine;
use bc_engine_text::{TextContent, TextMap, TextRules};
use bc_formats::registry::FormatRegistry;
use bc_formats::FormatKind;
use bc_report::{Comparison, Output, Renderer, ReportFile, ReportRequest, TextReportRenderer};
use bc_vfs::{VfsManager, VfsPath};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A loaded script ready to run.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    pub path: Option<PathBuf>,
    pub lines: Vec<ScriptLine>,
}

/// A failure that ends the process: the exit code plus the message for standard error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub code: ExitCode,
    pub message: String,
}

impl Failure {
    pub fn new(code: ExitCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Script {
    /// Read and parse a script file (`@file`): unreadable → 105, syntax → 106, a command
    /// this build does not run → 100. `args` are the arguments after the script name.
    pub fn load(path: &std::path::Path, args: &[String]) -> Result<Script, Failure> {
        let bytes = std::fs::read(path).map_err(|e| {
            Failure::new(
                ExitCode::ScriptLoadError,
                format!("cannot read script {}: {e}", path.display()),
            )
        })?;
        let source = Self::decode(&bytes).map_err(|e| {
            Failure::new(
                ExitCode::ScriptLoadError,
                format!("cannot read script {}: {e}", path.display()),
            )
        })?;
        let lines = crate::parser::parse_script(&source, args)
            .map_err(|e| Failure::new(e.exit_code(), format!("{}: {e}", path.display())))?;
        Ok(Script {
            path: Some(path.to_path_buf()),
            lines,
        })
    }

    /// Script text: a BOM selects UTF-8/UTF-16; without one the text is UTF-8, or the
    /// system ANSI code page when it is not valid UTF-8.
    pub fn decode(bytes: &[u8]) -> Result<String, String> {
        let detection = bc_core::encoding::detect(bytes, crate::platform::ansi_code_page());
        if detection.encoding == bc_core::encoding::Encoding::Binary {
            return Err("not a text file".into());
        }
        bc_core::encoding::decode(bytes, detection.encoding).map_err(|e| e.to_string())
    }
}

/// Runtime options from the command line that influence script execution.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunnerOptions {
    /// `/silent`: accepted; scripts always run without a window, so it changes nothing yet.
    pub silent: bool,
    /// `/closescript`: close the script window when done (GUI host).
    pub close_when_done: bool,
}

/// Where a running script writes: reports to `stdout` (`output-to:-`), errors to `stderr`.
/// The host passes the process streams; tests pass buffers.
pub struct Streams<'a> {
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
}

/// How a failed command counts towards the exit code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FailureKind {
    /// A compared file could not be read → 107.
    Load,
    /// Anything else (folder or binary input, report not written) → 100.
    Other,
}

/// Mutable interpreter state.
pub struct ScriptRunner {
    pub options: RunnerOptions,
    pub vfs: Arc<VfsManager>,
    pub formats: Arc<FormatRegistry>,
    /// Folder that relative paths in the script are resolved against.
    pub cwd: PathBuf,
    pub log_level: LogLevel,
    pub log_file: Option<PathBuf>,
    pub log_append: bool,
    pub stop_on_error: bool,
    pub confirm: Option<ScriptOption>,
    /// Open log file (opened when logging starts; reopened when the file changes).
    log: Option<std::fs::File>,
}

impl ScriptRunner {
    pub fn new(
        options: RunnerOptions,
        vfs: Arc<VfsManager>,
        formats: Arc<FormatRegistry>,
        cwd: PathBuf,
    ) -> Self {
        Self {
            options,
            vfs,
            formats,
            cwd,
            log_level: LogLevel::None,
            log_file: None,
            log_append: false,
            stop_on_error: false,
            confirm: None,
            log: None,
        }
    }

    /// Local file system, built-in plus user file formats, the process's current folder.
    pub fn headless(options: RunnerOptions) -> Self {
        Self::new(
            options,
            Arc::new(VfsManager::local_only()),
            Arc::new(default_formats()),
            std::env::current_dir().unwrap_or_default(),
        )
    }

    /// Execute every line; returns the process exit code: 0 when every command succeeded,
    /// 107 when a compared file could not be read, otherwise 100. Each failure is written to
    /// `stderr` (and the log); `option stop-on-error` ends the script at the first one.
    pub fn run(&mut self, script: &Script, io: &mut Streams) -> ExitCode {
        let mut failures = Vec::new();
        for line in &script.lines {
            let keyword = line.command.kind().keyword();
            self.log(LogLevel::Normal, &format!("line {}: {keyword}", line.line));
            self.log(
                LogLevel::Verbose,
                &format!("line {}: > {}", line.line, line.raw),
            );
            match self.execute(line, io) {
                Ok(()) => self.log(LogLevel::Normal, &format!("line {}: done", line.line)),
                Err((kind, message)) => {
                    // A real script names the line; a single command-line report does not.
                    let message = match script.path {
                        Some(_) => format!("line {}: {message}", line.line),
                        None => message,
                    };
                    let _ = writeln!(io.stderr, "{message}");
                    self.log(LogLevel::Normal, &format!("error: {message}"));
                    failures.push(kind);
                    if self.stop_on_error {
                        break;
                    }
                }
            }
        }
        if failures.contains(&FailureKind::Load) {
            ExitCode::ScriptLoadFileOrFolderFailed
        } else if failures.is_empty() {
            ExitCode::Success
        } else {
            ExitCode::UnknownError
        }
    }

    /// Execute one command.
    fn execute(
        &mut self,
        line: &ScriptLine,
        io: &mut Streams,
    ) -> Result<(), (FailureKind, String)> {
        match &line.command {
            Command::Beep => {
                crate::platform::beep();
                Ok(())
            }
            Command::Option(ScriptOption::StopOnError) => {
                self.stop_on_error = true;
                Ok(())
            }
            Command::Option(option) => {
                self.confirm = Some(*option);
                Ok(())
            }
            Command::Log {
                level,
                file,
                append,
            } => {
                if let Some(file) = file {
                    self.log_file = Some(self.resolve(file));
                    self.log_append = *append;
                    self.log = None;
                }
                if let Some(level) = level {
                    self.log_level = *level;
                }
                Ok(())
            }
            Command::TextReport(request) => self.text_report(request, io),
            Command::TableReport(request) => self.table_report(request, io),
            // The parser rejects every other command before the script runs.
            other => Err((
                FailureKind::Other,
                format!(
                    "the {} command is not supported yet",
                    other.kind().keyword()
                ),
            )),
        }
    }

    fn resolve(&self, path: &Path) -> PathBuf {
        let joined = self.cwd.join(path);
        std::path::absolute(&joined).unwrap_or(joined)
    }

    fn text_report(
        &mut self,
        request: &ReportRequest,
        io: &mut Streams,
    ) -> Result<(), (FailureKind, String)> {
        let Some(Comparison::Files(left, right)) = &request.comparison else {
            return Err((
                FailureKind::Other,
                "text-report needs two file names".into(),
            ));
        };
        let paths = [self.resolve(left), self.resolve(right)];
        let mut contents = Vec::with_capacity(2);
        let mut files = Vec::with_capacity(2);
        for path in &paths {
            if path.is_dir() {
                return Err((
                    FailureKind::Other,
                    format!(
                        "{} is a folder; text-report compares two files",
                        path.display()
                    ),
                ));
            }
            let read = || -> std::io::Result<(Vec<u8>, std::time::SystemTime)> {
                let vfs_path = VfsPath::local(path);
                let mut reader = self
                    .vfs
                    .resolve(&vfs_path)
                    .and_then(|fs| fs.open_read(&vfs_path))
                    .map_err(|e| std::io::Error::other(e.to_string()))?;
                let mut bytes = Vec::new();
                reader.read_to_end(&mut bytes)?;
                Ok((bytes, std::fs::metadata(path)?.modified()?))
            };
            let (bytes, modified) = read().map_err(|e| {
                (
                    FailureKind::Load,
                    format!("cannot read {}: {e}", path.display()),
                )
            })?;
            let format = self.formats.resolve(path, Some(FormatKind::Text)).cloned();
            let content = TextContent::from_bytes(bytes, format).map_err(|_| {
                (
                    FailureKind::Other,
                    format!("{} is not a text file", path.display()),
                )
            })?;
            contents.push(content);
            files.push(ReportFile {
                path: path.display().to_string(),
                modified: crate::platform::local(modified).patch_header(),
            });
        }
        let right = contents.pop().unwrap();
        let left = contents.pop().unwrap();
        let map = TextMap::compare(left, right, TextRules::default())
            .map_err(|e| (FailureKind::Other, format!("comparison failed: {e}")))?;
        let renderer = TextReportRenderer {
            map: &map,
            files: [files.remove(0), files.remove(0)],
        };
        let report = renderer
            .render(request)
            .map_err(|e| (FailureKind::Other, e.to_string()))?;
        self.deliver(&report, request, io)
    }

    /// `table-report layout:summary`: compare every sheet pair of two table files and
    /// write the readable cell-level report (`bc_report::table`).
    fn table_report(
        &mut self,
        request: &ReportRequest,
        io: &mut Streams,
    ) -> Result<(), (FailureKind, String)> {
        let Some(Comparison::Files(left, right)) = &request.comparison else {
            return Err((
                FailureKind::Other,
                "table-report needs two file names".into(),
            ));
        };
        let paths = [self.resolve(left), self.resolve(right)];
        for path in &paths {
            if path.is_dir() {
                return Err((
                    FailureKind::Other,
                    format!(
                        "{} is a folder; table-report compares two files",
                        path.display()
                    ),
                ));
            }
            if !path.is_file() {
                return Err((
                    FailureKind::Load,
                    format!("cannot read {}: file not found", path.display()),
                ));
            }
        }
        let sheets = crate::table_report::compare(
            &paths[0],
            &paths[1],
            &self.formats,
            request.options.ignore_unimportant,
        )
        .map_err(|e| {
            let kind = if e.load {
                FailureKind::Load
            } else {
                FailureKind::Other
            };
            (kind, e.message)
        })?;
        let renderer = bc_report::table::TableReportRenderer { sheets: &sheets };
        let report = renderer
            .render(request)
            .map_err(|e| (FailureKind::Other, e.to_string()))?;
        self.deliver(&report, request, io)
    }

    /// Write a rendered report to its `output-to:` target.
    fn deliver(
        &self,
        report: &bc_report::Report,
        request: &ReportRequest,
        io: &mut Streams,
    ) -> Result<(), (FailureKind, String)> {
        let output = match &request.output {
            Output::File(path) => Output::File(self.resolve(path)),
            other => other.clone(),
        };
        bc_report::deliver(report, &output, io.stdout).map_err(|e| {
            let target = match &output {
                Output::File(path) => path.display().to_string(),
                _ => "standard output".into(),
            };
            (
                FailureKind::Other,
                format!("cannot write the report to {target}: {e}"),
            )
        })
    }

    /// Append to the script log honouring `log` level/file: nothing until the level is
    /// raised from `none`; the default file is `Log.txt` in the script's folder (replaced),
    /// `append:` keeps the old content. A log file that cannot be opened is skipped:
    /// logging never fails a command (errors still reach standard error).
    pub fn log(&mut self, level: LogLevel, message: &str) {
        let enabled = match self.log_level {
            LogLevel::None => false,
            LogLevel::Normal => level == LogLevel::Normal,
            LogLevel::Verbose => true,
        };
        if !enabled {
            return;
        }
        if self.log.is_none() {
            let path = self
                .log_file
                .clone()
                .unwrap_or_else(|| self.cwd.join("Log.txt"));
            let mut options = std::fs::OpenOptions::new();
            options.create(true);
            if self.log_append {
                options.append(true);
            } else {
                options.write(true).truncate(true);
            }
            self.log = options.open(&path).ok();
            // Later messages in the same file append to what this run already wrote.
            self.log_append = true;
        }
        if let Some(file) = &mut self.log {
            let now = crate::platform::local(std::time::SystemTime::now());
            let _ = writeln!(
                file,
                "{} {:02}:{:02}:{:02}  {message}",
                now.date(),
                now.hour,
                now.minute,
                now.second
            );
        }
    }
}

impl Script {
    /// A one-command script without a file (the `/report` command-line shortcut): errors
    /// are reported without a line number.
    pub fn single(command: Command, raw: impl Into<String>) -> Script {
        Script {
            path: None,
            lines: vec![ScriptLine {
                line: 1,
                raw: raw.into(),
                command,
            }],
        }
    }

    /// Whether any command writes a report to standard output (`output-to:-`): the host
    /// must have a usable standard output before running it.
    pub fn writes_to_stdout(&self) -> bool {
        self.lines.iter().any(|line| {
            matches!(
                &line.command,
                Command::TextReport(request) | Command::TableReport(request)
                    if request.output == Output::Stdout
            )
        })
    }
}

/// Built-in plus user file formats (an unreadable user format folder falls back to the
/// built-ins, as the folder compare does).
pub fn default_formats() -> FormatRegistry {
    bc_formats::registry::user_formats_dir()
        .ok()
        .and_then(|dir| FormatRegistry::load_user_dir(&dir).ok())
        .unwrap_or_else(FormatRegistry::with_builtins)
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
    let formats = default_formats();
    // 规则比较按文件类型的会话规则：表格（xlsx/csv 等，判定见 `bc_formats::route`）走表格比较。
    let rules_based = mode.is_none_or(|m| m == bc_engine_dir::ContentCompare::RulesBased);
    if rules_based
        && bc_formats::route::pair_view(left, right, &formats) == bc_formats::route::FileView::Table
    {
        if let Some(code) = quick_compare_tables(left, right, &formats) {
            return Ok(code);
        }
    }
    let vfs = VfsManager::local_only();
    quick_compare(
        &vfs,
        &bc_vfs::VfsPath::local(left),
        &bc_vfs::VfsPath::local(right),
        mode,
        &formats,
    )
}

/// `/qc` 规则比较一对表格：字节相同或无差异 → `2`；只有不重要差异 → `12`；有重要差异、
/// 单侧工作表或结构列 → `13`。表格读不了时为 `None`，由调用方退回文本/字节比较。
fn quick_compare_tables(
    left: &std::path::Path,
    right: &std::path::Path,
    formats: &FormatRegistry,
) -> Option<ExitCode> {
    use bc_engine_table::compare::Importance;
    use bc_report::table::SheetComparison;
    if let (Ok(l), Ok(r)) = (std::fs::read(left), std::fs::read(right)) {
        if l == r {
            return Some(ExitCode::RulesBasedSame);
        }
    }
    let sheets = crate::table_report::compare(left, right, formats, false).ok()?;
    let mut unimportant = false;
    for sheet in &sheets {
        match sheet {
            SheetComparison::LeftOnly { .. } | SheetComparison::RightOnly { .. } => {
                return Some(ExitCode::RulesBasedDifferent)
            }
            SheetComparison::Compared { map, .. } => {
                let stats = map.statistics();
                match stats.importance() {
                    Importance::Important => return Some(ExitCode::RulesBasedDifferent),
                    _ if stats.structural_columns > 0 => {
                        return Some(ExitCode::RulesBasedDifferent)
                    }
                    Importance::Unimportant => unimportant = true,
                    Importance::Same => {}
                }
            }
        }
    }
    Some(if unimportant {
        ExitCode::Similar
    } else {
        ExitCode::RulesBasedSame
    })
}

/// One `/automerge` run: three input files, where to write, and the merge switches.
#[derive(Debug, Clone)]
pub struct AutoMergeRequest {
    pub left: PathBuf,
    pub right: PathBuf,
    pub center: PathBuf,
    pub output: PathBuf,
    /// `favor`, `ignore_unimportant`, `force_conflict_markers` from the switches; the
    /// rest stays at the defaults.
    pub rules: bc_engine_text::TextMergeRules,
    /// Marker labels (`/title1`, `/title2`, or the file names).
    pub labels: bc_engine_text::merge::MergeLabels,
}

impl AutoMergeRequest {
    /// From a parsed command line. `force` is separate because `/reviewconflicts`
    /// overrides `/force`; `labels` are the left and right marker labels. `None` when the
    /// merge has no output target.
    pub fn from_cli(
        spec: &crate::cli::MergeSpec,
        switches: crate::cli::MergeSwitches,
        force: bool,
        labels: [String; 2],
    ) -> Option<Self> {
        let [left, right] = labels;
        Some(Self {
            left: spec.left.clone(),
            right: spec.right.clone(),
            center: spec.center.clone(),
            output: spec.output.clone()?,
            rules: bc_engine_text::TextMergeRules {
                favor: switches.favor,
                ignore_unimportant: switches.ignore_unimportant,
                force_conflict_markers: force,
                ..Default::default()
            },
            labels: bc_engine_text::merge::MergeLabels { left, right },
        })
    }
}

/// What an `/automerge` run did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoMergeReport {
    /// 0, 14 or 101.
    pub code: ExitCode,
    /// Unresolved conflict sections.
    pub conflicts: usize,
    /// Whether the output file was written.
    pub written: bool,
    pub warnings: Vec<String>,
    /// Key columns a table merge used (`1 (inferred)`); `None` for text merges.
    pub keys: Option<String>,
}

impl AutoMergeReport {
    /// The one result line `/automerge` prints (not with `/silent`).
    pub fn summary(&self) -> String {
        let mut line = self.code.description().to_string();
        if self.conflicts > 0 {
            line.push_str(&format!(": {} conflict section(s)", self.conflicts));
        }
        if let Some(keys) = &self.keys {
            line.push_str(&format!(" (keys: {keys})"));
        }
        for warning in &self.warnings {
            line.push_str(&format!(" (warning: {warning})"));
        }
        line
    }
}

/// `/automerge <left> <right> <center> <output>` without a window (`TTextMergeSession`
/// automerge). Always a text merge: the file format is picked from the center file name
/// and the rules are the defaults, as for `/qc`.
///
/// * no conflicts → output written, `0`
/// * conflicts and `force_conflict_markers` → output written with markers, `14`
/// * conflicts otherwise → nothing written, an existing output file is left alone, `101`
///
/// The inputs are read completely before anything is written, and the output replaces
/// its target through a temporary file in the same folder, so it may be one of the
/// inputs. No backup is made and the previous content of the output is never read.
///
/// `Err` carries a message for unusable input (folder, missing, unreadable, not text),
/// unrepresentable output and write failures (the caller exits with `100`).
pub fn auto_merge_files(request: &AutoMergeRequest) -> Result<AutoMergeReport, String> {
    use bc_engine_text::{TextContent, TextMap};
    let formats = default_formats();
    let format = request
        .center
        .file_name()
        .and_then(|name| {
            formats.resolve(
                std::path::Path::new(name),
                Some(bc_formats::FormatKind::Text),
            )
        })
        .cloned();
    let load = |path: &std::path::Path| -> Result<TextContent, String> {
        if path.is_dir() {
            return Err(format!(
                "/automerge merges three files; {} is a folder",
                path.display()
            ));
        }
        if !path.is_file() {
            return Err(format!("/automerge: {} does not exist", path.display()));
        }
        let bytes =
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        TextContent::from_bytes(bytes, format.clone())
            .map_err(|_| format!("/automerge: {} is not a text file", path.display()))
    };
    let (left, right, center) = (
        load(&request.left)?,
        load(&request.right)?,
        load(&request.center)?,
    );
    let map = TextMap::merge(left, right, center, request.rules.text.clone())
        .map_err(|e| e.to_string())?;
    let outcome = bc_engine_text::merge::auto_merge(&map, &request.rules, &request.labels)
        .map_err(|e| format!("/automerge: {e}"))?;
    drop(map);
    let written = !outcome.withheld;
    if written {
        replace_file(&request.output, &outcome.output)?;
    }
    Ok(AutoMergeReport {
        code: auto_merge_exit_code(&outcome),
        conflicts: outcome.conflicts.len(),
        written,
        warnings: outcome.warnings,
        keys: None,
    })
}

/// Write `bytes` to a temporary file next to `path`, then rename it over `path`.
pub fn replace_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let failed = |e: std::io::Error| format!("cannot write {}: {e}", path.display());
    let name = path
        .file_name()
        .ok_or_else(|| format!("cannot write {}: not a file path", path.display()))?;
    let mut temp_name = std::ffi::OsString::from(".matcha-merge-");
    temp_name.push(std::process::id().to_string());
    temp_name.push("-");
    temp_name.push(name);
    temp_name.push(".tmp");
    let temp = path.with_file_name(temp_name);
    let result = (|| {
        let mut file = std::fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(failed)
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

#[cfg(test)]
mod load_tests {
    use super::*;

    fn write(tag: &str, bytes: &[u8]) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("matcha-script-{tag}-{}.txt", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        path
    }

    fn first_title(path: &std::path::Path) -> String {
        let script = Script::load(path, &[]).unwrap();
        let _ = std::fs::remove_file(path);
        match &script.lines[0].command {
            Command::TextReport(request) => request.title.clone().unwrap(),
            other => panic!("{other:?}"),
        }
    }

    const LINE: &str = "text-report layout:patch title:\"Ünïcode 比较\" output-to:- a b\r\n";

    #[test]
    fn utf8_with_and_without_bom() {
        let mut bom = b"\xef\xbb\xbf".to_vec();
        bom.extend_from_slice(LINE.as_bytes());
        assert_eq!(first_title(&write("bom", &bom)), "Ünïcode 比较");
        assert_eq!(
            first_title(&write("plain", LINE.as_bytes())),
            "Ünïcode 比较"
        );
    }

    #[test]
    fn utf16le_with_bom() {
        let mut bytes = vec![0xff, 0xfe];
        bytes.extend(LINE.encode_utf16().flat_map(u16::to_le_bytes));
        assert_eq!(first_title(&write("utf16", &bytes)), "Ünïcode 比较");
    }

    #[test]
    fn invalid_utf8_falls_back_to_the_ansi_code_page() {
        // 0xE9 is "é" in 1252 and invalid as UTF-8 on its own.
        let path = write(
            "ansi",
            b"text-report layout:patch title:caf\xe9 output-to:- a b\r\n",
        );
        let title = first_title(&path);
        assert!(
            title.starts_with("caf") && title.chars().count() == 4,
            "{title:?}"
        );
        if crate::platform::ansi_code_page() == 1252 {
            assert_eq!(title, "café");
        }
    }

    #[test]
    fn missing_script_is_105_and_syntax_errors_are_106() {
        let missing = std::env::temp_dir().join("matcha-no-such-script.txt");
        let failure = Script::load(&missing, &[]).unwrap_err();
        assert_eq!(failure.code, ExitCode::ScriptLoadError);
        assert!(failure.message.contains("matcha-no-such-script.txt"));
        let path = write("syntax", b"text-reprot layout:patch output-to:- a b\n");
        let failure = Script::load(&path, &[]).unwrap_err();
        let _ = std::fs::remove_file(&path);
        assert_eq!(failure.code, ExitCode::ScriptSyntaxError);
        assert!(failure.message.contains("line 1"), "{}", failure.message);
    }
}

#[cfg(test)]
mod run_tests {
    use super::*;
    use crate::parser::{parse_script_with, Variables};

    struct Run {
        code: ExitCode,
        stdout: Vec<u8>,
        stderr: String,
    }

    fn folder(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("matcha-run-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "one\ntwo\nthree\n").unwrap();
        std::fs::write(dir.join("b.txt"), "one\n2\nthree\n").unwrap();
        dir
    }

    fn run_in(dir: &Path, source: &str) -> Run {
        let lines = parse_script_with(source, &Variables::default()).unwrap();
        let script = Script {
            path: Some(dir.join("test-script.txt")),
            lines,
        };
        let mut runner = ScriptRunner::new(
            RunnerOptions::default(),
            Arc::new(VfsManager::local_only()),
            Arc::new(FormatRegistry::with_builtins()),
            dir.to_path_buf(),
        );
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let code = runner.run(
            &script,
            &mut Streams {
                stdout: &mut stdout,
                stderr: &mut stderr,
            },
        );
        Run {
            code,
            stdout,
            stderr: String::from_utf8(stderr).unwrap(),
        }
    }

    const UNIFIED: &str = "text-report layout:patch options:patch-unified output-to:- a.txt b.txt";

    #[test]
    fn report_to_stdout_succeeds() {
        let dir = folder("ok");
        let run = run_in(&dir, UNIFIED);
        let text = String::from_utf8(run.stdout).unwrap();
        assert_eq!(run.code, ExitCode::Success, "{}", run.stderr);
        assert!(run.stderr.is_empty());
        let a = dir.join("a.txt").display().to_string();
        assert!(text.starts_with(&format!("--- {a}\t")), "{text}");
        assert!(
            text.ends_with("@@ -1,3 +1,3 @@\r\n one\r\n-two\r\n+2\r\n three\r\n"),
            "{text}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn report_to_a_relative_file() {
        let dir = folder("file");
        let run = run_in(
            &dir,
            "text-report layout:patch output-to:out.patch a.txt b.txt",
        );
        assert_eq!(run.code, ExitCode::Success, "{}", run.stderr);
        assert!(run.stdout.is_empty());
        let written = std::fs::read_to_string(dir.join("out.patch")).unwrap();
        assert!(
            written.ends_with("2c2\r\n< two\r\n---\r\n> 2\r\n"),
            "{written}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_comparison_file_is_107() {
        let dir = folder("missing");
        let run = run_in(&dir, "text-report layout:patch output-to:- nope.txt b.txt");
        assert_eq!(run.code, ExitCode::ScriptLoadFileOrFolderFailed);
        assert!(
            run.stderr.starts_with("line 1: cannot read "),
            "{}",
            run.stderr
        );
        assert!(run.stderr.contains("nope.txt"));
        assert!(run.stdout.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn folders_and_binary_inputs_are_100() {
        let dir = folder("kinds");
        std::fs::create_dir(dir.join("sub")).unwrap();
        std::fs::write(dir.join("bin.dat"), b"\0\x01\x02\0\0\0").unwrap();
        let run = run_in(&dir, "text-report layout:patch output-to:- sub b.txt");
        assert_eq!(run.code, ExitCode::UnknownError);
        assert!(run.stderr.contains("is a folder"), "{}", run.stderr);
        let run = run_in(&dir, "text-report layout:patch output-to:- bin.dat b.txt");
        assert_eq!(run.code, ExitCode::UnknownError);
        assert!(run.stderr.contains("not a text file"), "{}", run.stderr);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn unwritable_report_is_100() {
        let dir = folder("unwritable");
        let run = run_in(
            &dir,
            "text-report layout:patch output-to:no-such-dir/r.patch a.txt b.txt",
        );
        assert_eq!(run.code, ExitCode::UnknownError);
        assert!(
            run.stderr.contains("cannot write the report"),
            "{}",
            run.stderr
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stop_on_error_ends_the_script_at_the_first_failure() {
        let dir = folder("stop");
        let failing = "text-report layout:patch output-to:- nope.txt b.txt";
        let without = run_in(&dir, &format!("{failing}\n{UNIFIED}"));
        assert_eq!(without.code, ExitCode::ScriptLoadFileOrFolderFailed);
        assert!(!without.stdout.is_empty(), "second report still runs");
        let with = run_in(&dir, &format!("option stop-on-error\n{failing}\n{UNIFIED}"));
        assert_eq!(with.code, ExitCode::ScriptLoadFileOrFolderFailed);
        assert!(with.stdout.is_empty(), "second report must not run");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stdout_reports_are_concatenated_in_order() {
        let dir = folder("concat");
        let one = run_in(&dir, UNIFIED).stdout;
        let normal = run_in(&dir, "text-report layout:patch output-to:- a.txt b.txt").stdout;
        let both = run_in(
            &dir,
            &format!("{UNIFIED}\ntext-report layout:patch output-to:- a.txt b.txt"),
        );
        assert_eq!(both.stdout, [one, normal].concat());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_defaults_to_log_txt_and_replaces_it() {
        let dir = folder("log-default");
        std::fs::write(dir.join("Log.txt"), "old run\n").unwrap();
        run_in(&dir, &format!("log normal\n{UNIFIED}"));
        let log = std::fs::read_to_string(dir.join("Log.txt")).unwrap();
        assert!(!log.contains("old run"), "{log}");
        assert!(
            log.contains("line 2: text-report") && log.contains("line 2: done"),
            "{log}"
        );
        assert!(
            !log.contains("> text-report"),
            "normal level does not echo commands"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn log_append_and_verbose() {
        let dir = folder("log-append");
        std::fs::write(dir.join("run.log"), "earlier\n").unwrap();
        run_in(&dir, &format!("log verbose append:run.log\n{UNIFIED}\ntext-report layout:patch output-to:- nope.txt b.txt"));
        let log = std::fs::read_to_string(dir.join("run.log")).unwrap();
        assert!(log.starts_with("earlier\n"), "{log}");
        assert!(log.contains(&format!("line 2: > {UNIFIED}")), "{log}");
        assert!(log.contains("error: line 3: cannot read"), "{log}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn logging_is_off_until_a_level_is_set() {
        let dir = folder("log-off");
        run_in(&dir, &format!("log quiet.log\n{UNIFIED}"));
        assert!(!dir.join("quiet.log").exists());
        assert!(!dir.join("Log.txt").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn beep_and_options_succeed_without_output() {
        let dir = folder("beep");
        let run = run_in(
            &dir,
            "beep\noption confirm:yes-to-all\noption stop-on-error",
        );
        assert_eq!(run.code, ExitCode::Success);
        assert!(run.stdout.is_empty() && run.stderr.is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn single_command_errors_carry_no_line_number() {
        let dir = folder("single");
        let request =
            crate::parser::shortcut_report("patch", None, None, "nope.txt", "b.txt").unwrap();
        let script = Script::single(Command::TextReport(request), "/report=patch nope.txt b.txt");
        let mut runner = ScriptRunner::new(
            RunnerOptions::default(),
            Arc::new(VfsManager::local_only()),
            Arc::new(FormatRegistry::with_builtins()),
            dir.clone(),
        );
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let code = runner.run(
            &script,
            &mut Streams {
                stdout: &mut stdout,
                stderr: &mut stderr,
            },
        );
        assert_eq!(code, ExitCode::ScriptLoadFileOrFolderFailed);
        let stderr = String::from_utf8(stderr).unwrap();
        assert!(stderr.starts_with("cannot read "), "{stderr}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scripts_know_whether_they_write_to_stdout() {
        let script = |source: &str| Script {
            path: None,
            lines: parse_script_with(source, &Variables::default()).unwrap(),
        };
        assert!(script(UNIFIED).writes_to_stdout());
        assert!(!script("text-report layout:patch output-to:r.txt a b").writes_to_stdout());
    }
}
