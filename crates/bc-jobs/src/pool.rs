//! Worker thread pool – BC `TJobThreadPool` / `TJobWorkerThread` / `TJobPriorityQueue`.
//!
//! `std::thread` workers pulling from a crossbeam channel per priority band.

use crate::{Job, JobContext, JobResult, Priority};
use std::thread::JoinHandle;

/// Work item as queued by the manager.
pub struct QueuedJob {
    pub job: Box<dyn Job>,
    pub ctx: JobContext,
    pub priority: Priority,
    /// Where the result is delivered.
    pub reply: crossbeam_channel::Sender<JobResult>,
}

/// Fixed-size pool of worker threads.
pub struct ThreadPool {
    workers: Vec<JoinHandle<()>>,
    /// One sender per priority band, highest first (`TJobPriorityQueue`).
    queues: Vec<crossbeam_channel::Sender<QueuedJob>>,
}

impl ThreadPool {
    /// Spawn `threads` workers.
    pub fn new(threads: usize) -> Self {
        todo!("bc-jobs::pool: spawn workers that select over priority queues")
    }

    /// Enqueue work.
    pub fn execute(&self, item: QueuedJob) {
        todo!("bc-jobs::pool: send to band queue")
    }

    /// Number of worker threads.
    pub fn len(&self) -> usize {
        self.workers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.workers.is_empty()
    }

    /// Number of priority bands (queues).
    pub fn bands(&self) -> usize {
        self.queues.len()
    }

    /// Close queues and join workers.
    pub fn join(self) {
        todo!("bc-jobs::pool: drop senders, join threads")
    }
}

impl std::fmt::Debug for ThreadPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ThreadPool").field("workers", &self.workers.len()).finish()
    }
}
