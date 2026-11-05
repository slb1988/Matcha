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
#![allow(unused_variables)] // scaffold: bodies are `todo!()`; remove once implemented
#![forbid(unsafe_code)]

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
}

/// `output-to:` target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    Printer,
    Clipboard,
    File(PathBuf),
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
}

/// Rendered report payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Report {
    Text(String),
    Html(String),
    Xml(String),
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

/// Text compare renderer (side-by-side / summary / interleaved / patch / statistics / xml).
pub struct TextReportRenderer<'a> {
    pub map: &'a bc_engine_text::TextMap,
}

impl Renderer for TextReportRenderer<'_> {
    fn render(&self, request: &ReportRequest) -> Result<Report, ReportError> {
        todo!("bc-report: text layouts")
    }
}

/// Folder compare renderer (side-by-side / summary / xml).
pub struct DirReportRenderer<'a> {
    pub tree: &'a bc_engine_dir::DirTree,
}

impl Renderer for DirReportRenderer<'_> {
    fn render(&self, request: &ReportRequest) -> Result<Report, ReportError> {
        todo!("bc-report: folder layouts")
    }
}

/// Deliver a rendered report – `TReportSaveToFileJob` / `TReportCopyToClipboardJob` / `TReportPrintJob`.
pub fn deliver(report: &Report, output: &Output) -> Result<(), ReportError> {
    todo!("bc-report: write file / clipboard / printer")
}
