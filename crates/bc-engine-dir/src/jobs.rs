//! Folder engine jobs – BC `TVfsLoadDirJob` (listing), `TQuickCompareFilesJob` /
//! `TDirTreeContentCompareJob` (content criteria, FR-D5), `TDirTreeContentSearchJob`
//! (find in folders), `TOpMoveBothJob` (file operations).

use crate::criteria::ContentCompare;
use crate::ops::DirOp;
use bc_jobs::{Job, JobContext, JobResult, Priority};
use bc_vfs::VfsPath;
use std::sync::Arc;

/// List one directory on one side.
pub struct LoadDirJob {
    pub vfs: Arc<dyn bc_vfs::Vfs>,
    pub side: usize,
    pub dir: VfsPath,
    pub relative: String,
    pub recursive: bool,
}

impl Job for LoadDirJob {
    fn name(&self) -> String {
        format!("Loading {}", self.dir)
    }

    fn priority(&self) -> Priority {
        Priority::High
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        todo!("bc-engine-dir::jobs: list directory (recursive when requested)")
    }
}

/// Compare the contents of one file pair – `TQuickCompareFilesJob`; also the backend of
/// `/qc=` and the `compare` script command.
pub struct ContentCompareJob {
    pub vfs: Arc<bc_vfs::VfsManager>,
    pub left: VfsPath,
    pub right: VfsPath,
    pub mode: ContentCompare,
    /// For rules-based compare: text rules + format registry.
    pub text_rules: Option<bc_engine_text::TextRules>,
    pub formats: Option<Arc<bc_formats::registry::FormatRegistry>>,
    pub ignore_unimportant: bool,
}

impl Job for ContentCompareJob {
    fn name(&self) -> String {
        "Comparing contents".into()
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        todo!("bc-engine-dir::jobs: size/CRC/binary (chunked, early exit) or rules-based via bc-engine-text")
    }
}

/// Find text inside files of the comparison – `TDirTreeContentSearchJob`.
pub struct ContentSearchJob {
    pub vfs: Arc<bc_vfs::VfsManager>,
    pub files: Vec<VfsPath>,
    pub options: bc_engine_text::search::FindOptions,
}

impl Job for ContentSearchJob {
    fn name(&self) -> String {
        "Searching".into()
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        todo!("bc-engine-dir::jobs: content search")
    }
}

/// Execute a file operation over a selection – `TOpMoveBothJob` and friends.
pub struct FileOpJob {
    pub vfs: Arc<bc_vfs::VfsManager>,
    pub op: DirOp,
    /// Relative paths selected, with their per-side absolute locations.
    pub items: Vec<(String, [Option<VfsPath>; 3])>,
}

impl Job for FileOpJob {
    fn name(&self) -> String {
        format!("{:?}", self.op)
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        todo!("bc-engine-dir::jobs: perform op with confirmations via ctx.interaction")
    }
}
