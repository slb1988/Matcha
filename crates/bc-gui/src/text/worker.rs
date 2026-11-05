//! 每视图一个可取消的 std worker；保存/卸载前 join，绝不泄漏 mmap 快照。
use bc_engine_text::jobs::AlignOutput;
use bc_jobs::{CancelToken, Job, JobContext, JobError, JobId, Progress, ProgressSink};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

#[derive(Default)]
pub struct ProgressState(Mutex<String>);
impl ProgressSink for ProgressState {
    fn report(&self, _: JobId, p: &Progress) {
        if let Ok(mut s) = self.0.lock() {
            s.clone_from(&p.message);
        }
    }
}
pub struct Worker {
    cancel: CancelToken,
    handle: Option<JoinHandle<Result<AlignOutput, JobError>>>,
    progress: Arc<ProgressState>,
}
impl Worker {
    pub fn spawn(job: Box<dyn Job>) -> Self {
        let cancel = CancelToken::new();
        let progress = Arc::new(ProgressState::default());
        let context = JobContext {
            id: JobId(1),
            cancel: cancel.clone(),
            progress: progress.clone(),
            interaction: None,
        };
        let handle = std::thread::spawn(move || {
            job.run(&context)?
                .downcast::<AlignOutput>()
                .map(|o| *o)
                .map_err(|_| JobError::Failed("unexpected alignment output".into()))
        });
        Self {
            cancel,
            handle: Some(handle),
            progress,
        }
    }
    pub fn progress(&self) -> String {
        self.progress
            .0
            .try_lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "Comparing".into())
    }
    pub fn poll(&mut self) -> Option<Result<AlignOutput, JobError>> {
        if !self.handle.as_ref()?.is_finished() {
            return None;
        }
        Some(
            self.handle
                .take()
                .unwrap()
                .join()
                .unwrap_or_else(|_| Err(JobError::Failed("comparison worker panicked".into()))),
        )
    }
    pub fn cancel(&self) {
        self.cancel.cancel();
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}
