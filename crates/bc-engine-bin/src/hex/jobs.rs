//! 对齐后台 job – 快照输入 + revision 过期拒绝（复用 text 视图的 worker 模式，设计 D4）。
//!
//! 任务 2.2：对齐在后台线程执行、可取消；job 快照携带两侧 revision，
//! 结果回填时 `HexMap::set_alignment` 对过期结果返回 `Err`，不污染视图模型。

use std::sync::Arc;

use bc_core::align::{AlignMap, ByteAlign};
use bc_core::cancel::CancelCheck;
use bc_core::CoreError;
use bc_jobs::{Job, JobContext, JobResult, Priority, Progress};

use super::content::HexContent;
use super::{HexMap, HexRules, HexSummary};

/// 一侧内容的可发送快照：小文件克隆字节、大文件共享只读映射。
#[derive(Debug, Clone)]
pub enum HexSnapshot {
    Whole(Arc<Vec<u8>>),
    Mapped(Arc<bc_core::mmap::ReadonlyMap>),
}

impl HexSnapshot {
    pub fn of(content: &HexContent) -> Self {
        match content.storage() {
            super::content::HexStorage::Whole(bytes) => Self::Whole(Arc::new(bytes.clone())),
            super::content::HexStorage::Mapped(map) => Self::Mapped(Arc::clone(map)),
        }
    }

    pub fn bytes(&self) -> &[u8] {
        match self {
            Self::Whole(bytes) => bytes,
            Self::Mapped(map) => map.bytes(),
        }
    }
}

/// 后台对齐产物。
#[derive(Debug)]
pub struct HexAlignOutput {
    pub map: AlignMap,
    /// 统计与差异游程（在后台线程随对齐算出，视图不必逐帧扫描对齐行）。
    pub summary: HexSummary,
    pub rules: HexRules,
    /// 快照时的两侧 revision；与当前内容不一致即过期。
    pub revisions: (u64, u64),
}

/// 后台对齐任务（设计 D4：编辑投递重对齐 job，携带 revision，过期结果丢弃）。
#[derive(Debug, Clone)]
pub struct HexAlignJob {
    pub left: HexSnapshot,
    pub right: HexSnapshot,
    pub rules: HexRules,
    pub revisions: (u64, u64),
}

impl HexAlignJob {
    pub fn from_map(map: &HexMap) -> Self {
        Self {
            left: HexSnapshot::of(&map.left),
            right: HexSnapshot::of(&map.right),
            rules: map.rules.clone(),
            revisions: (map.left.revision(), map.right.revision()),
        }
    }

    pub fn execute(&self, cancel: &dyn CancelCheck) -> Result<HexAlignOutput, CoreError> {
        cancel.check()?;
        let aligned = ByteAlign.align_cancellable(
            self.left.bytes(),
            self.right.bytes(),
            self.rules.alignment,
            cancel,
        )?;
        cancel.check()?;
        let summary = HexSummary::of(&aligned);
        Ok(HexAlignOutput {
            map: aligned,
            summary,
            rules: self.rules.clone(),
            revisions: self.revisions,
        })
    }
}

impl Job for HexAlignJob {
    fn name(&self) -> String {
        "Aligning bytes".into()
    }

    fn priority(&self) -> Priority {
        Priority::High
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        ctx.report(Progress {
            current: 0,
            total: Some(1),
            message: "Aligning bytes".into(),
        });
        let result = self.execute(&ctx.cancel).map_err(|error| match error {
            CoreError::Cancelled => bc_jobs::JobError::Cancelled,
            other => bc_jobs::JobError::Failed(other.to_string()),
        })?;
        ctx.report(Progress {
            current: 1,
            total: Some(1),
            message: "Comparison complete".into(),
        });
        Ok(Box::new(result))
    }
}
