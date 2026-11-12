//! # bc-report
//!
//! Report model and renderers (FR-R1, M2). One model serves every `*-report` script
//! command and the report dialogs. Options mirror the scripting reference exactly so the
//! script parser can map 1:1.
//!
//! | type | BC classes |
//! |---|---|
//! | [`ReportRequest`] | `TReportState` → `TSessionReportState` → `TFileReportState`/`TDirReportState` → `TTextReportState`, `TTableReportState`, `THexReportState`, `TTagReportState` (`TPixReportState`, `TMediaReportState`, `TVerReportState`), `TRegReportState`; `TMixedReportState` |
//! | [`Layout`] / [`Display`] | layout/display option sets of `TTextReportDialog`, `TDirReportDialog`, `THexReportDialog`, `TTableReportDialog`, … |
//! | [`Output`] / [`OutputOptions`] | `TReportSaveToFileJob`, `TReportCopyToClipboardJob`, `TReportPrintJob`, `TReportPreviewJob`, `TReportShowWebBrowserJob` |
//! | [`Report`] / [`Renderer`] | `TBcReport`, `TMetaReport`, `TMetaMultiReport`, `T*Layouter` (`THexSideBySideLayouter`, `THexInterleavedLayouter`, `THexSummaryLayouter`) |
#![forbid(unsafe_code)]

pub mod patch;
pub mod table;

use bc_session::SessionKind;
use std::path::PathBuf;

/// `layout:` values across all report commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Layout {
    SideBySide,
    Summary,
    /// Text/Table/Hex; replaces deprecated `over-under` and `composite`.
    Interleaved,
    /// Text only.
    Patch,
    /// Text only (always tabular CSV).
    Statistics,
    /// Text and Folder.
    Xml,
}

impl Layout {
    pub fn script_name(self) -> &'static str {
        match self {
            Layout::SideBySide => "side-by-side",
            Layout::Summary => "summary",
            Layout::Interleaved => "interleaved",
            Layout::Patch => "patch",
            Layout::Statistics => "statistics",
            Layout::Xml => "xml",
        }
    }

    /// Accepts deprecated spellings (`over-under`, `composite` → interleaved).
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s.to_ascii_lowercase().as_str() {
            "side-by-side" => Layout::SideBySide,
            "summary" => Layout::Summary,
            "interleaved" | "over-under" | "composite" => Layout::Interleaved,
            "patch" => Layout::Patch,
            "statistics" => Layout::Statistics,
            "xml" => Layout::Xml,
            _ => return None,
        })
    }
}

/// `display-*` option for file reports (folder reports use `bc_engine_dir::DisplayFilter`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Display {
    #[default]
    All,
    Mismatches,
    Matches,
    /// Text only.
    Context,
}

/// `patch-*` format for the patch layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PatchFormat {
    #[default]
    Normal,
    Context,
    Unified,
}

/// Report-type specific `options:` flags.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReportOptions {
    pub ignore_unimportant: bool,
    pub display: Display,
    pub line_numbers: bool,
    pub strikeout_left_diffs: bool,
    pub strikeout_right_diffs: bool,
    pub patch_format: PatchFormat,
    // folder-report
    pub folder_display: bc_engine_dir::DisplayFilter,
    /// `column-*` flags; empty = defaults (size, timestamp); `column-none` = explicit empty.
    pub columns: Option<Vec<String>>,
    pub include_file_links: bool,
    // table-report
    /// `title-row:<n>`（Matcha 扩展）：把源数据第 n 行（1 起，Excel 行号）的单元格当作列标题，
    /// 附在报告的列名后（`A[ID]`）。表头已被识别时该行就是列名本身，不再重复。
    pub title_row: Option<usize>,
}

/// `output-to:` target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Printer,
    Clipboard,
    File(PathBuf),
    /// `output-to:-` – the process's standard output. Matcha extension: the reference has
    /// no such target. The bytes are exactly those a file target would receive.
    Stdout,
}

/// `[<comparison>]` of the common report arguments: a pair of file names, or a saved
/// session name. Absent = the files selected within the script.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Comparison {
    Files(PathBuf, PathBuf),
    Session(String),
}

/// `output-options:` flags.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OutputOptions {
    pub print_color: bool,
    pub landscape: bool,
    pub wrap: Wrap,
    /// `html-color` / `html-mono` / `html-custom:<css>`; `None` = plain text.
    pub html: Option<HtmlStyle>,
}

/// Line wrapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Wrap {
    #[default]
    None,
    Character,
    Word,
}

/// HTML colour scheme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlStyle {
    Color,
    Mono,
    Custom(String),
}

/// Fully resolved report request – the `TReportState` family.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRequest {
    /// Session kind the report targets (`file-report` dispatches by file format).
    pub kind: SessionKind,
    pub layout: Layout,
    pub options: ReportOptions,
    pub title: Option<String>,
    pub output: Output,
    pub output_options: OutputOptions,
    pub comparison: Option<Comparison>,
}

/// Rendered report payload. A leading U+FEFF is part of the text: it becomes the UTF-8
/// BOM when the report is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    Text(String),
    Html(String),
    Xml(String),
}

impl Report {
    pub fn text(&self) -> &str {
        match self {
            Report::Text(text) | Report::Html(text) | Report::Xml(text) => text,
        }
    }

    /// The exact bytes written to a file or to standard output (UTF-8).
    pub fn to_bytes(&self) -> Vec<u8> {
        self.text().as_bytes().to_vec()
    }
}

/// Produces a [`Report`] from engine state – `TBcReport` + layouters.
pub trait Renderer {
    fn render(&self, request: &ReportRequest) -> Result<Report, ReportError>;
}

/// Errors while rendering or delivering.
#[derive(Debug)]
pub enum ReportError {
    UnsupportedLayout { kind: SessionKind, layout: Layout },
    Io(std::io::Error),
    Other(String),
}

impl std::fmt::Display for ReportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReportError::UnsupportedLayout { kind, layout } => {
                write!(f, "{layout:?} layout is not available for {kind:?}")
            }
            ReportError::Io(e) => write!(f, "io: {e}"),
            ReportError::Other(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for ReportError {}

/// How a compared file is named in report headers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReportFile {
    /// Path as the report prints it (absolute, platform separators).
    pub path: String,
    /// Modification time already formatted by the caller as the reference prints it
    /// (`YYYY-MM-DD HH:MM:SS.000000000 +hhmm`, local time); used by the context and
    /// unified patch headers.
    pub modified: String,
}

/// Text compare renderer. Only the `patch` layout is implemented; the others report
/// [`ReportError::UnsupportedLayout`].
pub struct TextReportRenderer<'a> {
    pub map: &'a bc_engine_text::TextMap,
    /// Left and right file as named in headers.
    pub files: [ReportFile; 2],
}

impl Renderer for TextReportRenderer<'_> {
    fn render(&self, request: &ReportRequest) -> Result<Report, ReportError> {
        match request.layout {
            Layout::Patch => Ok(Report::Text(patch::render_map(
                self.map,
                &self.files,
                request.options.patch_format,
                request.options.ignore_unimportant || self.map.rules.ignore_unimportant,
            ))),
            layout => Err(ReportError::UnsupportedLayout {
                kind: SessionKind::TextCompare,
                layout,
            }),
        }
    }
}

/// Folder compare renderer (side-by-side / summary / xml).
pub struct DirReportRenderer<'a> {
    pub tree: &'a bc_engine_dir::DirTree,
}

impl Renderer for DirReportRenderer<'_> {
    fn render(&self, _request: &ReportRequest) -> Result<Report, ReportError> {
        todo!("bc-report: folder layouts")
    }
}

/// Deliver a rendered report – `TReportSaveToFileJob` (file) or standard output.
///
/// * File: the bytes go to a temporary file beside the target, which then replaces the
///   target; on any failure the temporary file is removed, so no partial report is left.
/// * Standard output: written to `stdout` (the caller's real stdout, or a buffer in tests)
///   and flushed; a closed pipe is an [`ReportError::Io`], never a panic.
/// * Printer and clipboard are not supported yet.
pub fn deliver(
    report: &Report,
    output: &Output,
    stdout: &mut dyn std::io::Write,
) -> Result<(), ReportError> {
    let bytes = report.to_bytes();
    match output {
        Output::File(path) => write_file(path, &bytes).map_err(ReportError::Io),
        Output::Stdout => stdout
            .write_all(&bytes)
            .and_then(|()| stdout.flush())
            .map_err(ReportError::Io),
        Output::Printer => Err(ReportError::Other(
            "printer output is not supported yet".into(),
        )),
        Output::Clipboard => Err(ReportError::Other(
            "clipboard output is not supported yet".into(),
        )),
    }
}

fn write_file(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other(format!("{} is not a file name", path.display())))?;
    let temporary = path.with_file_name(format!(
        ".{}.matcha-{}.tmp",
        name.to_string_lossy(),
        std::process::id()
    ));
    let result = std::fs::write(&temporary, bytes).and_then(|()| std::fs::rename(&temporary, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deprecated_layouts_parse_as_interleaved() {
        for name in ["interleaved", "over-under", "composite", "OVER-UNDER"] {
            assert_eq!(Layout::parse(name), Some(Layout::Interleaved), "{name}");
        }
        assert_eq!(Layout::parse("patch"), Some(Layout::Patch));
        assert_eq!(Layout::parse("unified"), None);
        assert_eq!(Layout::Patch.script_name(), "patch");
    }

    #[test]
    fn comparison_and_stdout_target() {
        let files = Comparison::Files("a.txt".into(), "b.txt".into());
        assert_eq!(files, Comparison::Files("a.txt".into(), "b.txt".into()));
        assert_ne!(files, Comparison::Session("a.txt".into()));
        assert_ne!(Output::Stdout, Output::File("-".into()));
    }

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("matcha-deliver-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn file_delivery_replaces_an_existing_report() {
        let dir = scratch("overwrite");
        let path = dir.join("report.patch");
        std::fs::write(&path, "old content that is longer").unwrap();
        let report = Report::Text("new\r\n".into());
        deliver(&report, &Output::File(path.clone()), &mut Vec::new()).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new\r\n");
        assert_eq!(
            std::fs::read_dir(&dir).unwrap().count(),
            1,
            "no temporary file left"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_folder_fails_without_leaving_files() {
        let dir = scratch("missing");
        let target = dir.join("no-such-dir").join("r.patch");
        let error = deliver(
            &Report::Text("x".into()),
            &Output::File(target),
            &mut Vec::new(),
        );
        assert!(matches!(error, Err(ReportError::Io(_))));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stdout_receives_exactly_the_file_bytes() {
        let dir = scratch("stdout");
        let report = Report::Text("\u{feff}--- a\r\n+++ b\r\n".into());
        let path = dir.join("r.patch");
        deliver(&report, &Output::File(path.clone()), &mut Vec::new()).unwrap();
        let mut stdout = Vec::new();
        deliver(&report, &Output::Stdout, &mut stdout).unwrap();
        assert_eq!(stdout, std::fs::read(&path).unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn closed_pipe_is_an_error_not_a_panic() {
        struct Closed;
        impl std::io::Write for Closed {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let error = deliver(&Report::Text("x".into()), &Output::Stdout, &mut Closed);
        assert!(
            matches!(error, Err(ReportError::Io(e)) if e.kind() == std::io::ErrorKind::BrokenPipe)
        );
    }

    #[test]
    fn printer_and_clipboard_are_not_supported() {
        for output in [Output::Printer, Output::Clipboard] {
            let error = deliver(&Report::Text("x".into()), &output, &mut Vec::new()).unwrap_err();
            assert!(error.to_string().contains("not supported yet"), "{error}");
        }
    }

    #[test]
    fn report_bytes_are_utf8_with_the_bom_kept() {
        let report = Report::Text("\u{feff}é\r\n".into());
        assert_eq!(report.to_bytes(), b"\xef\xbb\xbf\xc3\xa9\r\n");
        assert_eq!(Report::Text("x".into()).to_bytes(), b"x");
    }
}
