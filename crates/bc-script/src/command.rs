//! The script commands (scripting_reference.html, alphabetical):
//! `ATTRIB BEEP COLLAPSE COMPARE COPY COPYTO CRITERIA DELETE EXPAND FILE-REPORT FILTER
//! FOLDER-REPORT HEX-REPORT LOAD LOG MEDIA-REPORT MOVE MOVETO OPTION PICTURE-REPORT
//! REGISTRY-REPORT RENAME SELECT SNAPSHOT SYNC TABLE-REPORT TEXT-REPORT TOUCH VERSION-REPORT`.
//!
//! Note: the analysis note says "28 commands" but the reference list above has **29**
//! entries; the enum follows the reference (see crate README, open questions).
//!
//! `lt`/`rt` are accepted wherever `left`/`right` are.

use bc_core::Side;
use bc_engine_dir::criteria::{ContentCompare, Criteria};
use bc_engine_dir::ops::{PathMode, RenameSpec, Target, TouchSpec};
use bc_engine_dir::sync::SyncMode;
use bc_engine_dir::{NameFilter, OtherFilters};
use bc_report::ReportRequest;
use bc_vfs::FileAttrs;
use std::path::PathBuf;

/// Command names as they appear in scripts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandKind {
    Attrib,
    Beep,
    Collapse,
    Compare,
    Copy,
    CopyTo,
    Criteria,
    Delete,
    Expand,
    FileReport,
    Filter,
    FolderReport,
    HexReport,
    Load,
    Log,
    MediaReport,
    Move,
    MoveTo,
    Option,
    PictureReport,
    RegistryReport,
    Rename,
    Select,
    Snapshot,
    Sync,
    TableReport,
    TextReport,
    Touch,
    VersionReport,
}

impl CommandKind {
    /// All commands in reference order (29 entries).
    pub const ALL: [CommandKind; 29] = [
        CommandKind::Attrib,
        CommandKind::Beep,
        CommandKind::Collapse,
        CommandKind::Compare,
        CommandKind::Copy,
        CommandKind::CopyTo,
        CommandKind::Criteria,
        CommandKind::Delete,
        CommandKind::Expand,
        CommandKind::FileReport,
        CommandKind::Filter,
        CommandKind::FolderReport,
        CommandKind::HexReport,
        CommandKind::Load,
        CommandKind::Log,
        CommandKind::MediaReport,
        CommandKind::Move,
        CommandKind::MoveTo,
        CommandKind::Option,
        CommandKind::PictureReport,
        CommandKind::RegistryReport,
        CommandKind::Rename,
        CommandKind::Select,
        CommandKind::Snapshot,
        CommandKind::Sync,
        CommandKind::TableReport,
        CommandKind::TextReport,
        CommandKind::Touch,
        CommandKind::VersionReport,
    ];

    /// Keyword as written in a script (lower-case canonical form).
    pub fn keyword(self) -> &'static str {
        match self {
            CommandKind::Attrib => "attrib",
            CommandKind::Beep => "beep",
            CommandKind::Collapse => "collapse",
            CommandKind::Compare => "compare",
            CommandKind::Copy => "copy",
            CommandKind::CopyTo => "copyto",
            CommandKind::Criteria => "criteria",
            CommandKind::Delete => "delete",
            CommandKind::Expand => "expand",
            CommandKind::FileReport => "file-report",
            CommandKind::Filter => "filter",
            CommandKind::FolderReport => "folder-report",
            CommandKind::HexReport => "hex-report",
            CommandKind::Load => "load",
            CommandKind::Log => "log",
            CommandKind::MediaReport => "media-report",
            CommandKind::Move => "move",
            CommandKind::MoveTo => "moveto",
            CommandKind::Option => "option",
            CommandKind::PictureReport => "picture-report",
            CommandKind::RegistryReport => "registry-report",
            CommandKind::Rename => "rename",
            CommandKind::Select => "select",
            CommandKind::Snapshot => "snapshot",
            CommandKind::Sync => "sync",
            CommandKind::TableReport => "table-report",
            CommandKind::TextReport => "text-report",
            CommandKind::Touch => "touch",
            CommandKind::VersionReport => "version-report",
        }
    }

    /// Parse a keyword (case-insensitive). `data-report` is the documented alias of
    /// `table-report`.
    pub fn parse(word: &str) -> Option<Self> {
        let w = word.to_ascii_lowercase();
        if w == "data-report" {
            return Some(CommandKind::TableReport);
        }
        Self::ALL.into_iter().find(|k| k.keyword() == w)
    }
}

/// Direction `left->right` / `right->left` (`lt`/`rt` accepted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Direction {
    pub from: Side,
    pub to: Side,
}

/// `load` forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadSpec {
    /// `load <session>`
    Session(String),
    /// `load [create:(all|left|right)] <left> [<right>]`
    Paths { left: String, right: Option<String>, create: Option<Target> },
    /// `load <default>`
    Default,
}

/// `log [none|normal|verbose] [[append:]<file>]`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogLevel {
    #[default]
    None,
    Normal,
    Verbose,
}

/// `option` forms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptOption {
    StopOnError,
    Confirm(bc_jobs::Confirmation),
    /// `confirm:prompt`
    ConfirmPrompt,
}

/// `select` mask: `[(left|right|all).][(exact|diff|newer|older|orphan|all).][(files|folders|all)]`
/// or `all` / `empty.folders`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectMask {
    pub side: Target,
    pub result: SelectResult,
    pub items: SelectItems,
    /// `empty.folders`.
    pub empty_folders: bool,
}

/// Comparison-result part of a select mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectResult {
    #[default]
    All,
    Exact,
    Diff,
    Newer,
    Older,
    Orphan,
}

/// Item-type part of a select mask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectItems {
    #[default]
    All,
    Files,
    Folders,
}

/// `snapshot` arguments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotSpec {
    pub save_crc: bool,
    pub save_version: bool,
    pub expand_archives: bool,
    pub follow_symlinks: bool,
    pub include_empty: bool,
    pub no_filters: bool,
    pub source: SnapshotSource,
    pub output: Option<PathBuf>,
}

/// `left|right|path:<path>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotSource {
    Left,
    Right,
    Path(PathBuf),
}

/// `filter` forms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterSpec {
    Names(NameFilter),
    /// cutoff / attrib / size / protected — merged into [`OtherFilters`].
    Other(OtherFilters),
}

/// A fully parsed command.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    /// `attrib (+|-)<set> […]`
    Attrib { set: FileAttrs, clear: FileAttrs },
    /// `beep`
    Beep,
    /// `collapse all | <path>…`
    Collapse { paths: Option<Vec<String>> },
    /// `compare [CRC|binary|rules-based]`
    Compare { mode: Option<ContentCompare> },
    /// `copy (left->right|right->left)`
    Copy(Direction),
    /// `copyto [left|right|all] [path:(relative|base|none)] <path>`
    CopyTo { target: Target, path_mode: PathMode, dest: String },
    /// `criteria …`
    Criteria(Criteria),
    /// `delete [recyclebin=(yes|no)] (left|right|all)`
    Delete { target: Target, recycle_bin: bool },
    /// `expand all | <path>…`
    Expand { paths: Option<Vec<String>> },
    /// `file-report …` (dispatches by file format)
    FileReport(ReportRequest),
    /// `filter …`
    Filter(FilterSpec),
    /// `folder-report …`
    FolderReport(ReportRequest),
    /// `hex-report …`
    HexReport(ReportRequest),
    /// `load …`
    Load(LoadSpec),
    /// `log …`
    Log { level: Option<LogLevel>, file: Option<PathBuf>, append: bool },
    /// `media-report …`
    MediaReport(ReportRequest),
    /// `move (left->right|right->left)`
    Move(Direction),
    /// `moveto …`
    MoveTo { target: Target, path_mode: PathMode, dest: String },
    /// `option …`
    Option(ScriptOption),
    /// `picture-report …`
    PictureReport(ReportRequest),
    /// `registry-report …`
    RegistryReport(ReportRequest),
    /// `rename [regexpr <old>] <new>`
    Rename(RenameSpec),
    /// `select <mask>…`
    Select { masks: Vec<SelectMask> },
    /// `snapshot …`
    Snapshot(SnapshotSpec),
    /// `sync [visible] [create-empty] (update|mirror):<direction>`
    Sync { mode: SyncMode, visible: bool, create_empty: bool },
    /// `table-report …` (alias `data-report`)
    TableReport(ReportRequest),
    /// `text-report …`
    TextReport(ReportRequest),
    /// `touch …`
    Touch(TouchSpec),
    /// `version-report …`
    VersionReport(ReportRequest),
}

impl Command {
    pub fn kind(&self) -> CommandKind {
        match self {
            Command::Attrib { .. } => CommandKind::Attrib,
            Command::Beep => CommandKind::Beep,
            Command::Collapse { .. } => CommandKind::Collapse,
            Command::Compare { .. } => CommandKind::Compare,
            Command::Copy(_) => CommandKind::Copy,
            Command::CopyTo { .. } => CommandKind::CopyTo,
            Command::Criteria(_) => CommandKind::Criteria,
            Command::Delete { .. } => CommandKind::Delete,
            Command::Expand { .. } => CommandKind::Expand,
            Command::FileReport(_) => CommandKind::FileReport,
            Command::Filter(_) => CommandKind::Filter,
            Command::FolderReport(_) => CommandKind::FolderReport,
            Command::HexReport(_) => CommandKind::HexReport,
            Command::Load(_) => CommandKind::Load,
            Command::Log { .. } => CommandKind::Log,
            Command::MediaReport(_) => CommandKind::MediaReport,
            Command::Move(_) => CommandKind::Move,
            Command::MoveTo { .. } => CommandKind::MoveTo,
            Command::Option(_) => CommandKind::Option,
            Command::PictureReport(_) => CommandKind::PictureReport,
            Command::RegistryReport(_) => CommandKind::RegistryReport,
            Command::Rename(_) => CommandKind::Rename,
            Command::Select { .. } => CommandKind::Select,
            Command::Snapshot(_) => CommandKind::Snapshot,
            Command::Sync { .. } => CommandKind::Sync,
            Command::TableReport(_) => CommandKind::TableReport,
            Command::TextReport(_) => CommandKind::TextReport,
            Command::Touch(_) => CommandKind::Touch,
            Command::VersionReport(_) => CommandKind::VersionReport,
        }
    }
}
