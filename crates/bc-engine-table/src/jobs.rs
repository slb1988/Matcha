use crate::{
    align::KeyAlign,
    map::{compare_alignment_data, TableAlignOutput, TableMap},
    store::SheetStore,
    Result, TableRules,
};
use bc_core::{align::AlignmentOverrides, cancel::CancelCheck, CoreError};
use bc_jobs::{Job, JobContext, JobResult, Progress};
#[derive(Debug, Clone)]
pub struct TableAlignJob {
    /// Immutable store snapshots (arc-shared with the live map; edits during
    /// the job copy-on-write, so the worker always sees a consistent version).
    pub left: SheetStore,
    pub right: SheetStore,
    pub rules: TableRules,
    pub overrides: AlignmentOverrides,
}
impl TableAlignJob {
    pub fn from_map(map: &TableMap) -> Self {
        Self {
            left: map.left.borrow().clone(),
            right: map.right.borrow().clone(),
            rules: map.rules().clone(),
            overrides: map.overrides(),
        }
    }
    pub fn execute(&self, cancel: &dyn CancelCheck) -> Result<TableAlignOutput> {
        let mut rules = self.rules.clone();
        let t = std::time::Instant::now();
        crate::map::prepare_rules_data(&self.left, &self.right, &mut rules, cancel)?;
        crate::perf::span("align:prepare-rules", t);
        let t = std::time::Instant::now();
        let alignment =
            KeyAlign.align_data(&self.left, &self.right, &rules, &self.overrides, cancel)?;
        crate::perf::span("align:key-align", t);
        let t = std::time::Instant::now();
        // Compare phase on demand storage supplies raw field text (prototype
        // fast path); the comparator's raw-text guard and normalization keep
        // typed semantics. Per-snapshot flag: the interactive store's typed
        // display is unaffected. Dense snapshots ignore the flag.
        self.left.set_compare_raw_supply(true);
        self.right.set_compare_raw_supply(true);
        let output = compare_alignment_data(
            &self.left,
            &self.right,
            &rules,
            &self.overrides,
            alignment,
            cancel,
        );
        self.left.set_compare_raw_supply(false);
        self.right.set_compare_raw_supply(false);
        crate::perf::span("align:compare-alignment", t);
        output
    }
}
fn error(e: CoreError) -> bc_jobs::JobError {
    match e {
        CoreError::Cancelled => bc_jobs::JobError::Cancelled,
        other => bc_jobs::JobError::Failed(other.to_string()),
    }
}
impl Job for TableAlignJob {
    fn name(&self) -> String {
        "Aligning table".into()
    }
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        ctx.report(Progress {
            current: 0,
            total: Some(1),
            message: "Hashing, aligning and comparing cells".into(),
        });
        let result = self.execute(&ctx.cancel).map_err(error)?;
        ctx.report(Progress {
            current: 1,
            total: Some(1),
            message: "Table comparison complete".into(),
        });
        Ok(Box::new(result))
    }
}
/// Safe full fallback: changed keys, order, mappings or settings can invalidate
/// any row pairing. No claimed incremental reuse until trustworthy anchors exist.
#[derive(Debug, Clone)]
pub struct TableRealignJob {
    pub base: TableAlignJob,
}
impl TableRealignJob {
    pub fn from_map(map: &TableMap) -> Self {
        Self {
            base: TableAlignJob::from_map(map),
        }
    }
    pub fn execute(&self, cancel: &dyn CancelCheck) -> Result<TableAlignOutput> {
        self.base.execute(cancel)
    }
}
impl Job for TableRealignJob {
    fn name(&self) -> String {
        "Re-aligning table".into()
    }
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        Ok(Box::new(self.execute(&ctx.cancel).map_err(error)?))
    }
}
