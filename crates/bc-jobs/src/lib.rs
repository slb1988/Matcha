//! # bc-jobs
//!
//! Background task framework – BC `TJob` / `TJobContext`.
//!
//! All I/O and comparison work in the application runs as jobs so the UI thread only
//! renders and edits (spec §7). Each job runs on its own `std` thread owned by the code
//! that started it ([`Worker`]); there is no shared pool or scheduler (ADR 008). No async
//! runtime.
//!
//! | type | BC class |
//! |---|---|
//! | [`Job`] | `TJob`, `TThreadJob`, `TSimpleJob` |
//! | [`JobContext`] / [`CancelToken`] / [`Progress`] | `TJobContext`, `TJobWaitForBlocker` |
//! | [`Worker`] | `TThreadJob` run on a dedicated thread |
#![forbid(unsafe_code)]

pub mod worker;

pub use worker::Worker;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Job identifier reported to [`ProgressSink`]s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(pub u64);

/// Declared priority (`TJobPriorityQueue`): interactive jobs (loading the visible pane)
/// outrank bulk content comparison. Informational while every job has its own thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub enum Priority {
    Low,
    #[default]
    Normal,
    High,
    /// User is waiting on it right now (`TJobWaitForBlocker`).
    Interactive,
}

/// Error returned by a job.
#[derive(Debug)]
pub enum JobError {
    Cancelled,
    Failed(String),
    Io(std::io::Error),
}

impl std::fmt::Display for JobError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobError::Cancelled => f.write_str("cancelled"),
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

    /// Execute. Must poll `ctx.cancel` regularly.
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult;
}
