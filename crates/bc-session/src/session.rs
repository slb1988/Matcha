//! Session model – BC `TSession` hierarchy.
//!
//! A session = kind + specs (what to compare) + rules (how) + state (view settings).
//! The XML class name is derived from [`SessionKind::bc_class`].
//!
//! Persistence keeps whatever the file carried: the original `<Rules>` element is
//! retained and only the keys this crate models (text alignment) are rewritten when
//! they change, so a session saved by the reference tool round-trips unchanged.

use crate::state::SessionState;
use bc_core::persist::Node;
use bc_vfs::VfsPath;

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

    /// Default rules of a new session of this kind.
    pub fn default_rules(self) -> SessionRules {
        match self {
            // Registry compare is line based in the reference (`TRegRules` ⊂ `TLineRules`).
            SessionKind::TextCompare
            | SessionKind::TextEdit
            | SessionKind::TextPatch
            | SessionKind::RegistryCompare => SessionRules::Text(Default::default()),
            SessionKind::TextMerge => SessionRules::TextMerge(Default::default()),
            SessionKind::TableCompare => SessionRules::Table(Default::default()),
            SessionKind::HexCompare => SessionRules::Hex(Default::default()),
            SessionKind::PictureCompare => SessionRules::Picture(Default::default()),
            SessionKind::MediaCompare | SessionKind::VersionCompare => {
                SessionRules::Tag(Default::default())
            }
            SessionKind::FolderCompare => SessionRules::Dir(Default::default()),
            SessionKind::FolderMerge => SessionRules::DirMerge(Default::default()),
            SessionKind::FolderSync => SessionRules::DirSync(Default::default()),
        }
    }
}

/// What a session compares – `<Specs>` element (`Left`, `Right`, `Center`, `Output`)
/// plus the `/title1..4` display aliases and `/vcs1..4` paths (not persisted).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionSpecs {
    pub left: Option<VfsPath>,
    pub right: Option<VfsPath>,
    pub center: Option<VfsPath>,
    pub output: Option<VfsPath>,
    pub titles: [Option<String>; 4],
    pub vcs_paths: [Option<String>; 4],
    pub read_only: [bool; 4],
    /// `<Specs>` children this crate does not model, kept for write-through.
    pub extra: Vec<Node>,
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

impl SessionRules {
    /// `Value` of the `<Rules>` element (Delphi rules class).
    pub fn bc_class(&self) -> &'static str {
        match self {
            SessionRules::Text(_) => "TTextRules",
            SessionRules::TextMerge(_) => "TTextMergeRules",
            SessionRules::Table(_) => "TTableRules",
            SessionRules::Hex(_) => "THexRules",
            SessionRules::Picture(_) => "TPixRules",
            SessionRules::Tag(_) => "TTagRules",
            SessionRules::Dir(_) => "TDirRules",
            SessionRules::DirMerge(_) => "TDirMergeRules",
            SessionRules::DirSync(_) => "TDirSyncRules",
        }
    }

    /// Text alignment settings, for the families that have them.
    pub fn alignment(&self) -> Option<&bc_engine_text::rules::AlignmentRules> {
        match self {
            SessionRules::Text(r) => Some(&r.alignment),
            SessionRules::TextMerge(r) => Some(&r.text.alignment),
            _ => None,
        }
    }

    pub fn alignment_mut(&mut self) -> Option<&mut bc_engine_text::rules::AlignmentRules> {
        match self {
            SessionRules::Text(r) => Some(&mut r.alignment),
            SessionRules::TextMerge(r) => Some(&mut r.text.alignment),
            _ => None,
        }
    }
}

/// `LastModified` timestamp: local wall-clock time without zone, exactly as the
/// reference writes it (`2026-10-30 20:47:37`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Stamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Stamp {
    /// Parse `YYYY-MM-DD HH:MM:SS`; `None` for anything else.
    pub fn parse(value: &str) -> Option<Stamp> {
        let (date, time) = value.trim().split_once(' ')?;
        let mut d = date.split('-');
        let mut t = time.split(':');
        let next = |it: &mut std::str::Split<'_, char>| it.next()?.parse::<u16>().ok();
        let stamp = Stamp {
            year: next(&mut d)?,
            month: u8::try_from(next(&mut d)?).ok()?,
            day: u8::try_from(next(&mut d)?).ok()?,
            hour: u8::try_from(next(&mut t)?).ok()?,
            minute: u8::try_from(next(&mut t)?).ok()?,
            second: u8::try_from(next(&mut t)?).ok()?,
        };
        let valid = d.next().is_none()
            && t.next().is_none()
            && (1..=12).contains(&stamp.month)
            && (1..=31).contains(&stamp.day)
            && stamp.hour < 24
            && stamp.minute < 60
            && stamp.second < 61;
        valid.then_some(stamp)
    }
}

impl std::fmt::Display for Stamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// A saved or transient session – `TSession`.
#[derive(Debug, Clone, PartialEq)]
pub struct Session {
    pub kind: SessionKind,
    /// Display name (`Value` attribute); `None` for the per-kind default sessions.
    pub name: Option<String>,
    /// `<LastModified Value="2026-10-30 20:47:37"/>`.
    pub last_modified: Option<Stamp>,
    pub specs: SessionSpecs,
    /// Typed rules; only the text alignment keys are read from / written to the file.
    pub rules: SessionRules,
    /// The `<Rules>` element as read, kept for write-through of unmodelled keys.
    pub rules_node: Option<Node>,
    pub state: SessionState,
    /// Elements we do not model (other than `Specs`/`Rules`/`State` children),
    /// retained for write-through.
    pub unknown: Vec<Node>,
}

impl Session {
    /// New session of `kind` with default rules for that kind.
    pub fn new(kind: SessionKind) -> Self {
        Self {
            kind,
            name: None,
            last_modified: None,
            specs: SessionSpecs::default(),
            rules: kind.default_rules(),
            rules_node: None,
            state: SessionState::default(),
            unknown: Vec::new(),
        }
    }

    /// Named two-sided session (`left` vs `right`).
    pub fn pair(kind: SessionKind, name: impl Into<String>, left: VfsPath, right: VfsPath) -> Self {
        let mut session = Self::new(kind);
        session.name = Some(name.into());
        session.specs.left = Some(left);
        session.specs.right = Some(right);
        session
    }

    pub fn is_saved(&self) -> bool {
        self.name.is_some()
    }
}
