//! Folder session rules – BC `TDirRules` → `TDirMergeRules` / `TDirSyncRules`, plus
//! handling options (`TDirSessionHandlingFrame`, `dirCompareFilesOnly` etc.).

use crate::criteria::Criteria;
use crate::filter::{NameFilter, OtherFilters};

/// How folders themselves are treated – `<DirHandling Value="dirCompareFilesOnly"/>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DirHandling {
    /// Compare files only; folder timestamps ignored.
    #[default]
    CompareFilesOnly,
    /// Also compare folder timestamps.
    CompareFolders,
}

/// Behaviour on copy/move – handling frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlingOptions {
    pub dir_handling: DirHandling,
    pub confirm_overwrite: bool,
    pub copy_timestamps: bool,
    pub copy_attributes: bool,
    pub follow_symlinks: bool,
    /// Treat archives as folders (M3+).
    pub archives_as_folders: bool,
    /// Compare names case-sensitively when pairing.
    pub case_sensitive_names: bool,
}

impl Default for HandlingOptions {
    fn default() -> Self {
        Self {
            dir_handling: DirHandling::CompareFilesOnly,
            confirm_overwrite: true,
            copy_timestamps: true,
            copy_attributes: true,
            follow_symlinks: false,
            archives_as_folders: false,
            case_sensitive_names: cfg!(not(windows)),
        }
    }
}

/// Folder Compare rules – `TDirRules`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirRules {
    pub criteria: Criteria,
    pub name_filter: NameFilter,
    pub other_filters: OtherFilters,
    pub handling: HandlingOptions,
    /// Per-extension format overrides (`TDirFileFormatList`).
    pub format_overrides: Vec<(String, String)>,
}

/// Folder Merge rules – `TDirMergeRules` (adds center handling).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirMergeRules {
    pub dir: DirRules,
}

/// Folder Sync rules – `TDirSyncRules` (adds default sync mode).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DirSyncRules {
    pub dir: DirRules,
    pub mode: crate::sync::SyncMode,
    pub create_empty_folders: bool,
    /// "Only affect visible files".
    pub visible_only: bool,
}
