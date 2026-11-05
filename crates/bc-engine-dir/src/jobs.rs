//! Folder engine jobs – BC `TVfsLoadDirJob` (listing), `TQuickCompareFilesJob` /
//! `TDirTreeContentCompareJob` (content criteria, FR-D5), `TDirTreeContentSearchJob`
//! (find in folders), `TOpMoveBothJob` (file operations).

use crate::ops::DirOp;
use bc_jobs::{Job, JobContext, JobResult, Priority};
use bc_vfs::VfsPath;
use std::sync::Arc;

pub use crate::content::ContentResult;

#[derive(Debug)]
pub struct DirectoryListing {
    pub side: usize,
    pub relative: String,
    pub entries: Vec<bc_vfs::VfsEntry>,
}
fn job_error(error: bc_vfs::VfsError) -> bc_jobs::JobError {
    match error {
        bc_vfs::VfsError::Cancelled => bc_jobs::JobError::Cancelled,
        other => bc_jobs::JobError::Failed(other.to_string()),
    }
}

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
        let mut result = Vec::new();
        let mut pending = vec![(self.dir.clone(), self.relative.clone())];
        let mut visited = std::collections::HashSet::new();
        while let Some((path, relative)) = pending.pop() {
            ctx.cancel.check()?;
            if let Some(local) = path.to_local_path() {
                let canonical = std::fs::canonicalize(local)?;
                if !visited.insert(canonical) {
                    continue;
                }
            }
            let entries = self.vfs.list(&path).map_err(job_error)?;
            if self.recursive {
                for entry in &entries {
                    if entry.kind == bc_vfs::EntryKind::Dir && entry.error.is_none() {
                        pending.push((
                            path.join(&entry.name),
                            if relative.is_empty() {
                                entry.name.clone()
                            } else {
                                format!("{relative}/{}", entry.name)
                            },
                        ));
                    }
                }
            }
            result.push(DirectoryListing {
                side: self.side,
                relative,
                entries,
            });
            ctx.report(bc_jobs::Progress {
                current: result.len() as u64,
                total: None,
                message: path.to_string(),
            });
        }
        Ok(Box::new(result))
    }
}

/// Compare the contents of one file pair – `TQuickCompareFilesJob`; also the backend of
/// `/qc=` and the `compare` script command.
pub struct ContentCompareJob {
    pub vfs: Arc<bc_vfs::VfsManager>,
    pub left: VfsPath,
    pub right: VfsPath,
    pub criteria: crate::Criteria,
    pub quick_status: crate::CompareStatus,
    pub node: crate::tree::NodeId,
    pub revision: u64,
}

impl Job for ContentCompareJob {
    fn name(&self) -> String {
        "Comparing contents".into()
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        let result = self.compare(&ctx.cancel).map_err(job_error)?;
        ctx.report(bc_jobs::Progress {
            current: result.bytes_read.iter().sum(),
            total: None,
            message: "Content comparison complete".into(),
        });
        Ok(Box::new(result))
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
