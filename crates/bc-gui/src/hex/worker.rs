//! 每视图一个可取消的 std worker；与 text 视图同约定：保存/卸载前 join，绝不泄漏 mmap 快照。
use bc_engine_bin::hex::{HexAlignJob, HexAlignOutput};
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

/// 后台对齐 worker：携带 revision 的快照 job（过期结果由 `HexMap::set_alignment` 拒绝）。
pub struct Worker {
    cancel: CancelToken,
    handle: Option<JoinHandle<Result<HexAlignOutput, JobError>>>,
    progress: Arc<ProgressState>,
}

impl Worker {
    pub fn spawn(job: HexAlignJob) -> Self {
        let cancel = CancelToken::new();
        let progress = Arc::new(ProgressState::default());
        let context = JobContext {
            id: JobId(1),
            cancel: cancel.clone(),
            progress: progress.clone(),
            interaction: None,
        };
        let handle = std::thread::spawn(move || {
            Box::new(job)
                .run(&context)?
                .downcast::<HexAlignOutput>()
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

    pub fn poll(&mut self) -> Option<Result<HexAlignOutput, JobError>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use bc_engine_bin::hex::{HexContent, HexMap, HexStats};

    /// 后台执行 → poll 收取 → set_alignment 回填（revision 一致时）。
    #[test]
    fn worker_roundtrip_applies_fresh_output() {
        // 非退化内容（全零内容里单字节翻转会被锚点机制表示为孤儿对——合法对齐）；
        // 确定性伪随机基底上有明确的差异点。
        let mut seed = 0xC0FFEEu64;
        let mut rand = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 32) as u8
        };
        let left: Vec<u8> = (0..256).map(|_| rand()).collect();
        let mut right = left.clone();
        right[0x80] ^= 0xFF;
        let mut map = HexMap {
            left: HexContent::from_bytes(left),
            right: HexContent::from_bytes(right),
            ..Default::default()
        };
        let job = HexAlignJob::from_map(&map);
        let mut worker = Worker::spawn(job);
        let output = loop {
            if let Some(result) = worker.poll() {
                break result.unwrap();
            }
            std::thread::yield_now();
        };
        map.set_alignment(output).unwrap();
        assert_eq!(map.stats(), HexStats { same: 255, difference: 1, ..Default::default() });
    }

    /// 过期结果不污染视图模型：快照后编辑 → set_alignment 拒绝。
    #[test]
    fn stale_output_is_dropped() {
        let mut map = HexMap {
            left: HexContent::from_bytes(vec![0u8; 64]),
            right: HexContent::from_bytes(vec![0u8; 64]),
            ..Default::default()
        };
        let job = HexAlignJob::from_map(&map);
        let mut worker = Worker::spawn(job);
        map.right.replace(0..0, &[0xFF]).unwrap(); // 编辑触发 revision 前进
        let output = loop {
            if let Some(result) = worker.poll() {
                break result.unwrap();
            }
            std::thread::yield_now();
        };
        assert!(map.set_alignment(output).is_err());
        assert!(map.align.is_empty()); // 旧对齐未被覆盖
    }
}
