use crate::{
    align::KeyAlign,
    map::{compare_alignment, TableAlignOutput, TableMap},
    Result, Sheet, TableRules,
};
use bc_core::{align::AlignmentOverrides, cancel::CancelCheck, CoreError};
use bc_jobs::{Job, JobContext, JobResult, Progress};
#[derive(Debug, Clone)]
pub struct TableAlignJob {
    pub left: Sheet,
    pub right: Sheet,
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
        crate::map::prepare_rules(&self.left, &self.right, &mut rules, cancel)?;
        let alignment = KeyAlign.align(&self.left, &self.right, &rules, &self.overrides, cancel)?;
        compare_alignment(
            &self.left,
            &self.right,
            &rules,
            &self.overrides,
            alignment,
            cancel,
        )
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
