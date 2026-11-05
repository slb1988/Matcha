//! # bc-jobs
//!
//! Background task framework – BC `TJob` / `TJobManager` / `TJobThreadPool` /
//! `TJobResourcePool` / `TJobContext` / `TJobPriorityQueue` / `TJobDependencyArray`.
//!
//! All I/O and comparison work in the application runs as jobs so the UI thread only
//! renders and edits (spec §7). Implementation constraint: `std::thread` + crossbeam
//! channels only – no async runtime.
//!
//! | type | BC class |
//! |---|---|
//! | [`Job`] | `TJob`, `TThreadJob`, `TSimpleJob`, `TResourceJob` |
//! | [`JobContext`] / [`CancelToken`] / [`Progress`] | `TJobContext`, `TJobWaitForBlocker` |
//! | [`JobManager`] / [`JobHandle`] | `TJobManager` (`TAsyncThreadSynchronizer`), `TJobArray` |
//! | [`pool::ThreadPool`] | `TJobThreadPool`, `TJobWorkerThread`, `TJobPriorityQueue` |
//! | [`resource::ResourcePool`] | `TJobResourcePool`, `TJobResource`, `TJobResourceThread` |
#![allow(unused_variables)] // scaffold: bodies are `todo!()`; remove once implemented
#![forbid(unsafe_code)]

pub mod manager;
pub mod pool;
pub mod resource;

pub use manager::{JobHandle, JobManager, JobManagerConfig};
pub use resource::{ResourceKey, ResourceLease, ResourcePool};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Unique job identifier within a [`JobManager`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(pub u64);

/// Scheduling priority (`TJobPriorityQueue`). Interactive jobs (loading the visible
/// pane) outrank bulk content comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
    /// User is waiting on it right now (`TJobWaitForBlocker`).
    Interactive,
}

/// Lifecycle state visible through [`JobHandle::status`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobStatus {
    /// Queued, possibly waiting on dependencies.
    Pending,
    Running,
    Completed,
    Failed(String),
    Cancelled,
}

/// Error returned by a job.
#[derive(Debug)]
pub enum JobError {
    Cancelled,
    /// Dependency failed, so this job never ran.
    DependencyFailed(JobId),
    Failed(String),
    Io(std::io::Error),
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobError::Cancelled => f.write_str("cancelled"),
            JobError::DependencyFailed(id) => write!(f, "dependency {id:?} failed"),
            JobError::Failed(m) => f.write_str(m),
            JobError::Io(e) => write!(f, "io: {e}"),
        }
    }
}

impl std::error::Error for JobError {}

impl From<std::io::Error> for JobError {
    fn from(e: std::io::Error) -> Self {
        JobError::Io(e)
    }
}

/// Result of running a job. Payloads are type-erased so the manager stays generic;
/// typed wrappers live in the engine crates.
pub type JobResult = Result<Box<dyn std::any::Any + Send>, JobError>;

/// Cooperative cancellation flag shared between a job and its handle.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// Request cancellation.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether cancellation was requested.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    /// Convenience for job bodies: `ctx.cancel.check()?;`
    pub fn check(&self) -> Result<(), JobError> {
        if self.is_cancelled() {
            Err(JobError::Cancelled)
        } else {
            Ok(())
        }
    }
}

impl bc_core::cancel::CancelCheck for CancelToken {
    fn is_cancelled(&self) -> bool { self.is_cancelled() }
}

/// Progress snapshot published by a job.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Progress {
    pub current: u64,
    /// `None` when the total is unknown (indeterminate bar).
    pub total: Option<u64>,
    pub message: String,
}

/// Receives progress updates (UI status bar, script log).
pub trait ProgressSink: Send + Sync {
    fn report(&self, job: JobId, progress: &Progress);
}

/// Per-run context handed to [`Job::run`] – BC `TJobContext`.
#[derive(Clone)]
pub struct JobContext {
    pub id: JobId,
    pub cancel: CancelToken,
    pub progress: Arc<dyn ProgressSink>,
    /// Interactive questions (overwrite? retry?) – `TDirInteractiveContext`; `None` under `/silent`.
    pub interaction: Option<Arc<dyn Interaction>>,
}

impl JobContext {
    /// Publish progress.
    pub fn report(&self, progress: Progress) {
        self.progress.report(self.id, &progress);
    }
}

/// User interaction hook for jobs that may need confirmation (file operations).
pub trait Interaction: Send + Sync {
    /// Ask a yes/no/yes-to-all/no-to-all question; returns `None` when non-interactive.
    fn confirm(&self, question: &str) -> Option<Confirmation>;
}

/// Answer to [`Interaction::confirm`] – matches script `option confirm:`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Confirmation {
    Yes,
    No,
    YesToAll,
    NoToAll,
}

/// A unit of background work – BC `TJob`.
pub trait Job: Send + 'static {
    /// Display name for the jobs dialog (`TJobsCancelDialog`).
    fn name(&self) -> String;

    fn priority(&self) -> Priority {
        Priority::Normal
    }

    /// Resources this job must lease before running (connections, disks) – `TResourceJob`.
    fn resources(&self) -> Vec<ResourceKey> {
        Vec::new()
    }

    /// Execute. Must poll `ctx.cancel` regularly.
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult;
}
