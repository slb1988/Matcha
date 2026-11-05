//! Filters – BC `TDirFilters` (name masks, `TDirOtherFilterList`), the `filter` script
//! command, the `/filters=` switch and the toolbar display filter (FR-D2).

use bc_vfs::FileAttrs;
use std::time::SystemTime;

/// One mask from a semicolon separated list: `*.pas`, `-*.bak`, `My Folder\`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    pub pattern: String,
    /// Leading `-` → exclude.
    pub exclude: bool,
    /// Trailing `\` (or `/`) → applies to folders.
    pub folder: bool,
}

/// Name filter (`filter "<masks>"`, `/filters=`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NameFilter {
    pub masks: Vec<Mask>,
}

impl NameFilter {
    /// Parse `*.c;*.h;-*.bak;obj\`.
    pub fn parse(spec: &str) -> NameFilter {
        todo!("bc-engine-dir::filter: parse mask list")
    }

    /// Whether a relative path (file or folder) passes.
    pub fn matches(&self, relative: &str, is_dir: bool) -> bool {
        todo!("bc-engine-dir::filter: include/exclude evaluation (help topic file_masks)")
    }
}

/// `cutoff:` date filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cutoff {
    /// Exclude files older than.
    OlderThan(SystemTime),
    /// Exclude files newer than.
    NewerThan(SystemTime),
}

/// `size:` filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SizeLimit {
    SmallerThan(u64),
    LargerThan(u64),
}

/// Other filters – `TDirSessionOtherFiltersFrame`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OtherFilters {
    pub cutoff: Option<Cutoff>,
    /// Attributes that must be set (`+`) / must not be set (`-`).
    pub attrib_include: Option<FileAttrs>,
    pub attrib_exclude: Option<FileAttrs>,
    pub size: Option<SizeLimit>,
    /// `exclude-protected`: skip system+hidden items.
    pub exclude_protected: bool,
}

/// Toolbar display filter – matches the `folder-report` `display-*` options and the
/// `select` comparison classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DisplayFilter {
    #[default]
    All,
    Mismatches,
    NoOrphans,
    MismatchesNoOrphans,
    Orphans,
    LeftNewer,
    RightNewer,
    LeftNewerOrphans,
    RightNewerOrphans,
    LeftOrphans,
    RightOrphans,
    Matches,
}

impl DisplayFilter {
    /// Script/report spelling (`display-mismatches-no-orphans`).
    pub fn script_name(self) -> &'static str {
        match self {
            DisplayFilter::All => "display-all",
            DisplayFilter::Mismatches => "display-mismatches",
            DisplayFilter::NoOrphans => "display-no-orphans",
            DisplayFilter::MismatchesNoOrphans => "display-mismatches-no-orphans",
            DisplayFilter::Orphans => "display-orphans",
            DisplayFilter::LeftNewer => "display-left-newer",
            DisplayFilter::RightNewer => "display-right-newer",
            DisplayFilter::LeftNewerOrphans => "display-left-newer-orphans",
            DisplayFilter::RightNewerOrphans => "display-right-newer-orphans",
            DisplayFilter::LeftOrphans => "display-left-orphans",
            DisplayFilter::RightOrphans => "display-right-orphans",
            DisplayFilter::Matches => "display-matches",
        }
    }

    /// Whether a node with `status` is shown.
    pub fn shows(self, status: crate::CompareStatus) -> bool {
        use crate::CompareStatus::*;
        match self {
            Self::All => true,
            Self::Mismatches => !matches!(status, Same | Unknown),
            Self::Matches => status == Same,
            Self::NoOrphans => !matches!(status, LeftOrphan | RightOrphan),
            Self::MismatchesNoOrphans => matches!(status, Different | LeftNewer | RightNewer | SimilarUnimportant | Error),
            Self::Orphans => matches!(status, LeftOrphan | RightOrphan),
            Self::LeftNewer => status == LeftNewer,
            Self::RightNewer => status == RightNewer,
            Self::LeftNewerOrphans => matches!(status, LeftNewer | LeftOrphan),
            Self::RightNewerOrphans => matches!(status, RightNewer | RightOrphan),
            Self::LeftOrphans => status == LeftOrphan,
            Self::RightOrphans => status == RightOrphan,
        }
    }
}
