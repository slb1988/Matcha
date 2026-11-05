//! Background alignment jobs – BC `TTextMapJob` → `TTextMapAlign2Job`,
//! `TTextMapRealign2Job`, `TTextMapAlign3BaseJob` → `TTextMapAlign3Job`, `TTextMapRealign3Job`.
//!
//! Each job owns a snapshot of the inputs (hashes, weights, unimportant flags) so the UI
//! thread can keep editing; the result is applied through [`crate::TextMap::set_alignment`].

use crate::map::TextMapRow;
use crate::rules::TextRules;
use bc_core::align::{AlignMap, AlignStrategy, LineWeight};
use bc_core::hash::ContentHash;
use bc_jobs::{Job, JobContext, JobResult, Priority};

/// Snapshot of one side prepared on the UI thread.
#[derive(Debug, Clone, Default)]
pub struct SideSnapshot {
    pub hashes: Vec<ContentHash>,
    pub weights: Vec<LineWeight>,
    pub unimportant: Vec<bool>,
}

/// Result payload delivered through [`JobResult`].
#[derive(Debug)]
pub struct AlignOutput {
    pub align: AlignMap,
    pub rows: Vec<TextMapRow>,
}

/// Full 2-way alignment – `TTextMapAlign2Job`.
#[derive(Debug)]
pub struct Align2Job {
    pub left: SideSnapshot,
    pub right: SideSnapshot,
    pub strategy: AlignStrategy,
    pub rules: TextRules,
}

impl Job for Align2Job {
    fn name(&self) -> String {
        "Aligning text".into()
    }

    fn priority(&self) -> Priority {
        Priority::High
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        todo!("bc-engine-text::jobs: hash → Aligner::align2 → classify → AlignOutput")
    }
}

/// Incremental re-alignment after an edit – `TTextMapRealign2Job`.
#[derive(Debug)]
pub struct Realign2Job {
    pub base: Align2Job,
    /// Row range invalidated by the edit.
    pub dirty_rows: std::ops::Range<usize>,
}

impl Job for Realign2Job {
    fn name(&self) -> String {
        "Re-aligning text".into()
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        todo!("bc-engine-text::jobs: windowed realign")
    }
}

/// Center-based 3-way alignment – `TTextMapAlign3Job`.
#[derive(Debug)]
pub struct Align3Job {
    pub left: SideSnapshot,
    pub center: SideSnapshot,
    pub right: SideSnapshot,
    pub strategy: AlignStrategy,
    pub rules: TextRules,
}

impl Job for Align3Job {
    fn name(&self) -> String {
        "Aligning merge".into()
    }

    fn priority(&self) -> Priority {
        Priority::High
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        todo!("bc-engine-text::jobs: align3")
    }
}

/// Incremental 3-way re-alignment – `TTextMapRealign3Job`.
#[derive(Debug)]
pub struct Realign3Job {
    pub base: Align3Job,
    pub dirty_rows: std::ops::Range<usize>,
}

impl Job for Realign3Job {
    fn name(&self) -> String {
        "Re-aligning merge".into()
    }

    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        todo!("bc-engine-text::jobs: windowed realign3")
    }
}
