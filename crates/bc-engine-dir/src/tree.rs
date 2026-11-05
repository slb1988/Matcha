//! Dual (or triple) folder tree – BC `TDirTree` on `TSsMapTree`; `TDirSyncTree` adds
//! per-node sync actions. Each node pairs the same relative name across sides.

use bc_vfs::VfsEntry;

/// Comparison outcome of one node (drives colours, `select` masks and display filters).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CompareStatus {
    /// Not compared yet (content compare pending).
    #[default]
    Unknown,
    Same,
    /// Differs by non-timestamp criteria (`select diff`).
    Different,
    LeftNewer,
    RightNewer,
    LeftOrphan,
    RightOrphan,
    /// Differences only in unimportant text (rules-based compare).
    SimilarUnimportant,
    /// Could not be compared (error / access denied).
    Error,
}

/// Node in the tree – one row of the folder view.
#[derive(Debug, Clone, Default)]
pub struct DirNode {
    /// Name relative to the parent.
    pub name: String,
    /// `[left, right, center]`.
    pub sides: [Option<VfsEntry>; 3],
    pub status: CompareStatus,
    pub children: Vec<DirNode>,
    pub expanded: bool,
    /// Excluded by filters but kept for "show excluded".
    pub excluded: bool,
    pub is_dir: bool,
}

impl DirNode {
    pub fn is_orphan(&self) -> bool {
        matches!(self.status, CompareStatus::LeftOrphan | CompareStatus::RightOrphan)
    }

    /// Recursively count files by status (summary report).
    pub fn count_status(&self, status: CompareStatus) -> usize {
        todo!("bc-engine-dir::tree: recursive count")
    }

    /// Find child by relative path (`expand "My Folder/Sub"`).
    pub fn find(&self, relative: &str) -> Option<&DirNode> {
        todo!("bc-engine-dir::tree: path lookup")
    }
}

/// Root of a folder comparison.
#[derive(Debug, Default)]
pub struct DirTree {
    pub root: DirNode,
    /// Base specs `[left, right, center]` as loaded.
    pub bases: [Option<bc_vfs::VfsPath>; 3],
    /// Manual alignment overrides (`TDirAlignWithList`): left name ↔ right name.
    pub align_overrides: Vec<(String, String)>,
}

impl DirTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// Merge a freshly listed directory (`TVfsLoadDirJob` result) into the tree at `relative`.
    pub fn apply_listing(&mut self, side: usize, relative: &str, entries: Vec<VfsEntry>) {
        todo!("bc-engine-dir::tree: merge listing, pair by name (case rules), apply align overrides")
    }

    /// Re-evaluate statuses that can be derived without content (size/time/attrib).
    pub fn evaluate_quick(&mut self, criteria: &crate::Criteria) {
        todo!("bc-engine-dir::tree: quick criteria pass")
    }

    /// Iterate nodes in display order honouring expansion and `filter`.
    pub fn visible<'a>(&'a self, filter: &'a crate::DisplayFilter) -> Vec<&'a DirNode> {
        todo!("bc-engine-dir::tree: flatten visible rows")
    }

    /// Expand/collapse all or specific paths (`expand` / `collapse` commands).
    pub fn set_expanded(&mut self, paths: Option<&[String]>, expanded: bool) {
        todo!("bc-engine-dir::tree: expand/collapse")
    }
}
