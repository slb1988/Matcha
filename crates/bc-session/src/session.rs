//! Session model – BC `TSession` hierarchy.
//!
//! A session = kind + specs (what to compare) + rules (how) + state (view settings).
//! The XML class name is derived from [`SessionKind::bc_class`].

use crate::state::SessionState;
use bc_vfs::VfsPath;
use std::time::SystemTime;

/// Every session/view type of the reference tool (feature inventory table).
/// Also the value set of the `/fv=` switch (see [`SessionKind::force_view_name`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionKind {
    TextCompare,
    TextMerge,
    TextEdit,
    TextPatch,
    TableCompare,
    HexCompare,
    PictureCompare,
    MediaCompare,
    RegistryCompare,
    VersionCompare,
    FolderCompare,
    FolderMerge,
    FolderSync,
}

impl SessionKind {
    pub const ALL: [SessionKind; 13] = [
        SessionKind::TextCompare,
        SessionKind::TextMerge,
        SessionKind::TextEdit,
        SessionKind::TextPatch,
        SessionKind::TableCompare,
        SessionKind::HexCompare,
        SessionKind::PictureCompare,
        SessionKind::MediaCompare,
        SessionKind::RegistryCompare,
        SessionKind::VersionCompare,
        SessionKind::FolderCompare,
        SessionKind::FolderMerge,
        SessionKind::FolderSync,
    ];

    /// Delphi class used as the XML element name in `BCSessions.xml`.
    pub fn bc_class(self) -> &'static str {
        match self {
            SessionKind::TextCompare => "TTextCompareSession",
            SessionKind::TextMerge => "TTextMergeSession",
            SessionKind::TextEdit => "TTextEditSession",
            SessionKind::TextPatch => "TTextPatchSession",
            SessionKind::TableCompare => "TTableCompareSession",
            SessionKind::HexCompare => "THexCompareSession",
            SessionKind::PictureCompare => "TPixCompareSession",
            SessionKind::MediaCompare => "TMediaCompareSession",
            SessionKind::RegistryCompare => "TRegCompareSession",
            SessionKind::VersionCompare => "TVerCompareSession",
            SessionKind::FolderCompare => "TDirCompareSession",
            SessionKind::FolderMerge => "TDirMergeSession",
            SessionKind::FolderSync => "TDirSyncSession",
        }
    }

    pub fn from_bc_class(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.bc_class() == name)
    }

    /// Spelling accepted by `/fv=<type>`.
    pub fn force_view_name(self) -> &'static str {
        match self {
            SessionKind::TextCompare => "Text Compare",
            SessionKind::TextMerge => "Text Merge",
            SessionKind::TextEdit => "Text Edit",
            SessionKind::TextPatch => "Text Patch",
            SessionKind::TableCompare => "Table Compare",
            SessionKind::HexCompare => "Hex Compare",
            SessionKind::PictureCompare => "Picture Compare",
            SessionKind::MediaCompare => "Media Compare",
            SessionKind::RegistryCompare => "Registry Compare",
            SessionKind::VersionCompare => "Version Compare",
            SessionKind::FolderCompare => "Folder Compare",
            SessionKind::FolderMerge => "Folder Merge",
            SessionKind::FolderSync => "Folder Sync",
        }
    }

    pub fn from_force_view_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.force_view_name().eq_ignore_ascii_case(name.trim()))
    }

    /// Folder-based kinds use `TDirRules`; the rest are file sessions.
    pub fn is_folder(self) -> bool {
        matches!(self, SessionKind::FolderCompare | SessionKind::FolderMerge | SessionKind::FolderSync)
    }
}

/// What a session compares – `<Specs>` element (`Left`, `Right`, `Center`, `Output`)
/// plus the `/title1..4` display aliases and `/vcs1..4` paths.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionSpecs {
    pub left: Option<VfsPath>,
    pub right: Option<VfsPath>,
    pub center: Option<VfsPath>,
    pub output: Option<VfsPath>,
    pub titles: [Option<String>; 4],
    pub vcs_paths: [Option<String>; 4],
    pub read_only: [bool; 4],
}

/// Rules variant per session family.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionRules {
    Text(bc_engine_text::TextRules),
    TextMerge(bc_engine_text::TextMergeRules),
    Table(bc_engine_table::TableRules),
    Hex(bc_engine_bin::hex::HexRules),
    Picture(bc_engine_bin::pix::PixRules),
    Tag(bc_engine_bin::tag::TagRules),
    Dir(bc_engine_dir::DirRules),
    DirMerge(bc_engine_dir::rules::DirMergeRules),
    DirSync(bc_engine_dir::rules::DirSyncRules),
}

/// A saved or transient session – `TSession`.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub kind: SessionKind,
    /// Display name (`Value` attribute); `None` for unsaved "Untitled" sessions.
    pub name: Option<String>,
    pub specs: SessionSpecs,
    pub rules: SessionRules,
    pub state: SessionState,
    /// `<LastModified Value="2026-10-30 20:47:37"/>`.
    pub last_modified: Option<SystemTime>,
    /// Child sessions of a folder session (`TDirChildSessionList`) – per-file overrides.
    pub children: Vec<Session>,
    /// Elements we do not model, retained for write-through.
    pub unknown: Vec<bc_core::persist::Node>,
}

impl Session {
    /// New session of `kind` with default rules for that kind.
    pub fn new(kind: SessionKind) -> Self {
        todo!("bc-session::session: default rules per kind")
    }

    /// Infer the session kind from positional CLI arguments
    /// (2 folders → Folder Compare, 2 files → by format, 3 files → Text Merge, 4 → merge + output).
    pub fn infer_kind(specs: &SessionSpecs, formats: &bc_formats::registry::FormatRegistry) -> SessionKind {
        todo!("bc-session::session: kind inference")
    }

    pub fn is_saved(&self) -> bool {
        self.name.is_some()
    }
}
