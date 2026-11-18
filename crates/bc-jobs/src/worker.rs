//! One cancellable job on its own `std` thread, owned by whoever spawned it (ADR 008).
//!
//! The owner polls [`Worker::poll`] (the GUI does so every frame) and reads the latest
//! progress message. Dropping a worker cancels the job and joins its thread, so a job
//! never outlives its owner — views rely on this to release memory-mapped snapshots
//! before saving over or unloading a file.
use crate::{CancelToken, Job, JobContext, JobError, JobId, Progress, ProgressSink};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

/// Latest progress message of a job.
#[derive(Default)]
struct LastMessage(Mutex<String>);

impl ProgressSink for LastMessage {
    fn report(&self, _: JobId, progress: &Progress) {
        if let Ok(mut message) = self.0.lock() {
            message.clone_from(&progress.message);
        }
    }
}

/// A running (or finished, not yet collected) job whose output has type `T`.
pub struct Worker<T: Send + 'static> {
    cancel: CancelToken,
    handle: Option<JoinHandle<Result<T, JobError>>>,
    progress: Arc<LastMessage>,
    _output: PhantomData<fn() -> T>,
}

impl<T: Send + 'static> Worker<T> {
    /// Start `job` on a new thread. Its boxed output must be a `T`.
    pub fn spawn(job: Box<dyn Job>) -> Self {
        let cancel = CancelToken::new();
        let progress = Arc::new(LastMessage::default());
        let context = JobContext {
            id: JobId(1),
            cancel: cancel.clone(),
            progress: progress.clone(),
            interaction: None,
        };
        let handle = std::thread::spawn(move || {
            let name = job.name();
            job.run(&context)?
                .downcast::<T>()
                .map(|output| *output)
                .map_err(|_| JobError::Failed(format!("{name}: unexpected output type")))
        });
        Self {
            cancel,
            handle: Some(handle),
            progress,
            _output: PhantomData,
        }
    }

    /// The job's result once it has finished (`None` while running or once collected).
    /// A panicking job is reported as [`JobError::Failed`].
    pub fn poll(&mut self) -> Option<Result<T, JobError>> {
        if !self.handle.as_ref()?.is_finished() {
            return None;
        }
        let handle = self.handle.take()?;
        Some(
            handle
                .join()
                .unwrap_or_else(|_| Err(JobError::Failed("background job panicked".into()))),
        )
    }

    /// `true` until the result has been collected by [`Worker::poll`].
    pub fn is_pending(&self) -> bool {
        self.handle.is_some()
    }

    /// Ask the job to stop; it returns [`JobError::Cancelled`] at its next check.
    pub fn cancel(&self) {
        self.cancel.cancel();
    }

    /// Latest progress message (empty before the first report; "Comparing" while the
    /// job is updating it).
    pub fn progress(&self) -> String {
        self.progress
            .0
            .try_lock()
            .map(|message| message.clone())
            .unwrap_or_else(|_| "Comparing".into())
    }
}

impl<T: Send + 'static> Drop for Worker<T> {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JobResult;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct Count {
        to: u64,
        started: Arc<AtomicBool>,
    }
    impl Job for Count {
        fn name(&self) -> String {
            "count".into()
        }
        fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
            self.started.store(true, Ordering::SeqCst);
            let mut n = 0;
            while n < self.to {
                ctx.cancel.check()?;
                n += 1;
                ctx.report(Progress {
                    current: n,
                    total: Some(self.to),
                    message: format!("step {n}"),
                });
                if self.to == u64::MAX {
                    std::thread::yield_now();
                }
            }
            Ok(Box::new(n))
        }
    }
    struct Panics;
    impl Job for Panics {
        fn name(&self) -> String {
            "panics".into()
        }
        fn run(self: Box<Self>, _: &JobContext) -> JobResult {
            panic!("boom")
        }
    }

    fn wait<T: Send + 'static>(worker: &mut Worker<T>) -> Result<T, JobError> {
        loop {
            if let Some(result) = worker.poll() {
                return result;
            }
            std::thread::yield_now();
        }
    }

    #[test]
    fn returns_the_typed_output_once() {
        let started = Arc::new(AtomicBool::new(false));
        let mut worker = Worker::<u64>::spawn(Box::new(Count { to: 3, started }));
        assert_eq!(wait(&mut worker).unwrap(), 3);
        assert!(!worker.is_pending());
        assert!(worker.poll().is_none());
        assert_eq!(worker.progress(), "step 3");
    }

    #[test]
    fn cancel_and_drop_stop_the_job() {
        let started = Arc::new(AtomicBool::new(false));
        let mut worker = Worker::<u64>::spawn(Box::new(Count {
            to: u64::MAX,
            started: started.clone(),
        }));
        while !started.load(Ordering::SeqCst) {
            std::thread::yield_now();
        }
        worker.cancel();
        assert!(matches!(wait(&mut worker), Err(JobError::Cancelled)));
        // Dropping a running worker cancels and joins (the test would hang otherwise).
        let running = Worker::<u64>::spawn(Box::new(Count {
            to: u64::MAX,
            started: Arc::new(AtomicBool::new(false)),
        }));
        drop(running);
    }

    #[test]
    fn wrong_output_type_and_panics_are_failures() {
        let started = Arc::new(AtomicBool::new(false));
        let mut worker = Worker::<String>::spawn(Box::new(Count { to: 1, started }));
        assert!(matches!(wait(&mut worker), Err(JobError::Failed(m)) if m.contains("count")));
        let mut worker = Worker::<u64>::spawn(Box::new(Panics));
        assert!(matches!(wait(&mut worker), Err(JobError::Failed(_))));
        assert_eq!(worker.progress(), "");
    }
}
