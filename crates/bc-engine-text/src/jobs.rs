//! Send-only snapshot jobs. No job manager or asynchronous runtime is required.
use crate::{
    content::ContentSnapshot,
    importance::{CompiledComparison, Importance, LineView},
    map::{AlignedData, CompactRow, TextMap},
    rules::TextRules,
};
use bc_core::{
    align::{
        AlignInput, AlignMap, AlignStrategy, AlignmentOverrides, LineWeight, QuickAlign,
        SmartAlign, SmartAlignOptions,
    },
    cancel::CancelCheck,
    hash::{ContentHash, LineHasher},
    pattern::Pattern,
    CoreError, DiffKind, Side,
};
use bc_jobs::{Job, JobContext, JobResult, Priority, Progress};
use std::{ops::Range, sync::Arc};

pub type SideSnapshot = ContentSnapshot;
#[derive(Debug, Clone)]
pub struct AlignOutput {
    pub(crate) data: Arc<AlignedData>,
    pub rules: TextRules,
    pub overrides: AlignmentOverrides,
    pub recomputed_rows: Range<usize>,
}
impl AlignOutput {
    pub fn row_count(&self) -> usize {
        self.data.rows.len()
    }
}
#[derive(Debug, Clone)]
pub struct Align2Job {
    pub left: SideSnapshot,
    pub right: SideSnapshot,
    pub strategy: AlignStrategy,
    pub rules: TextRules,
    pub overrides: AlignmentOverrides,
}
impl Align2Job {
    pub fn from_map(map: &TextMap) -> Self {
        Self {
            left: map.left.snapshot(),
            right: map.right.snapshot(),
            strategy: map.rules.alignment.strategy,
            rules: map.rules.clone(),
            overrides: map.overrides.clone(),
        }
    }
    pub fn execute(&self, cancel: &dyn CancelCheck) -> Result<AlignOutput, CoreError> {
        cancel.check()?;
        let comparison = CompiledComparison::new(&self.rules.comparison)?;
        let lh = hash_side(&self.left, Side::Left, &comparison, cancel)?;
        let rh = hash_side(&self.right, Side::Right, &comparison, cancel)?;
        let align = align_range(self, &lh, &rh, 0..lh.len(), 0..rh.len(), cancel)?;
        let rows = classify_alignment(align, &self.left, &self.right, 0, 0, &comparison, cancel)?;
        let len = rows.len();
        let mut data = AlignedData {
            rows,
            sections: Vec::new(),
            left_hashes: Arc::new(lh),
            right_hashes: Arc::new(rh),
            revisions: (self.left.revision(), self.right.revision()),
            rules: self.rules.clone(),
            overrides: self.overrides.clone(),
        };
        data.rebuild_sections();
        cancel.check()?;
        Ok(AlignOutput {
            data: Arc::new(data),
            rules: self.rules.clone(),
            overrides: self.overrides.clone(),
            recomputed_rows: 0..len,
        })
    }
}
impl Job for Align2Job {
    fn name(&self) -> String {
        "Aligning text".into()
    }
    fn priority(&self) -> Priority {
        Priority::High
    }
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        ctx.report(Progress {
            current: 0,
            total: Some(1),
            message: "Hashing, aligning and classifying".into(),
        });
        let result = self.execute(&ctx.cancel).map_err(job_error)?;
        ctx.report(Progress {
            current: 1,
            total: Some(1),
            message: "Comparison complete".into(),
        });
        Ok(Box::new(result))
    }
}
fn job_error(error: CoreError) -> bc_jobs::JobError {
    match error {
        CoreError::Cancelled => bc_jobs::JobError::Cancelled,
        other => bc_jobs::JobError::Failed(other.to_string()),
    }
}
fn hasher(comparison: &CompiledComparison) -> LineHasher {
    let r = &comparison.rules.importance;
    LineHasher {
        ignore_leading_whitespace: r.leading_whitespace_unimportant,
        ignore_embedded_whitespace: r.embedded_whitespace_unimportant,
        ignore_trailing_whitespace: r.trailing_whitespace_unimportant,
        ignore_case: r.case_unimportant,
    }
}
fn hash_line(
    side: &SideSnapshot,
    index: usize,
    which: Side,
    comparison: &CompiledComparison,
    cancel: &dyn CancelCheck,
) -> Result<ContentHash, CoreError> {
    let text = std::str::from_utf8(
        side.lines
            .line_bytes(index)
            .ok_or_else(|| CoreError::Invalid("line index out of range".into()))?,
    )
    .map_err(|e| CoreError::Invalid(e.to_string()))?;
    let hasher = hasher(comparison);
    if !comparison.needs_semantic_hash(side.format.as_deref()) {
        return Ok(hasher.hash_line(text.as_bytes()));
    }
    let tokens = side
        .syntax
        .tokens_checked(&side.lines, index, cancel, false)?;
    let normalized = comparison.alignment_text(
        LineView {
            text,
            tokens: &tokens,
            format: side.format.as_deref(),
            ending: side.lines.ending(index).unwrap(),
        },
        which,
    );
    Ok(hasher.hash_line(normalized.as_bytes()))
}
fn hash_side(
    side: &SideSnapshot,
    which: Side,
    comparison: &CompiledComparison,
    cancel: &dyn CancelCheck,
) -> Result<Vec<ContentHash>, CoreError> {
    let mut result = Vec::with_capacity(side.lines.len());
    for i in 0..side.lines.len() {
        if i % 128 == 0 {
            cancel.check()?;
        }
        result.push(hash_line(side, i, which, comparison, cancel)?);
    }
    Ok(result)
}
fn weights(
    side: &SideSnapshot,
    range: Range<usize>,
    rules: &TextRules,
    cancel: &dyn CancelCheck,
) -> Result<Vec<LineWeight>, CoreError> {
    let extra = rules
        .alignment
        .weights
        .iter()
        .map(|w| {
            let pattern = if w.is_regexp {
                Pattern::new(&w.find_text, true)
            } else {
                Pattern::literal(&w.find_text, true)
            }?;
            Ok((pattern, LineWeight(w.priority.0)))
        })
        .collect::<Result<Vec<_>, CoreError>>()?;
    let mut output = Vec::with_capacity(range.len());
    for i in range {
        if i % 128 == 0 {
            cancel.check()?;
        }
        let text = std::str::from_utf8(side.lines.line_bytes(i).unwrap())
            .map_err(|e| CoreError::Invalid(e.to_string()))?;
        let base = side.syntax.line_weight(text);
        output.push(
            extra
                .iter()
                .filter(|(p, _)| p.find_at(text, 0).is_some())
                .map(|(_, w)| *w)
                .fold(base, LineWeight::max),
        );
    }
    Ok(output)
}
fn align_range(
    job: &Align2Job,
    lh: &[ContentHash],
    rh: &[ContentHash],
    lr: Range<usize>,
    rr: Range<usize>,
    cancel: &dyn CancelCheck,
) -> Result<AlignMap, CoreError> {
    let left = job.left.lines.slice(lr.clone());
    let right = job.right.lines.slice(rr.clone());
    let lw = weights(&job.left, lr.clone(), &job.rules, cancel)?;
    let rw = weights(&job.right, rr.clone(), &job.rules, cancel)?;
    let mut overrides = AlignmentOverrides::default();
    for &(l, r) in job.overrides.pairs() {
        if lr.contains(&l) && rr.contains(&r) {
            overrides.insert(l - lr.start, r - rr.start)?;
        } else if lr.contains(&l) || rr.contains(&r) {
            return Err(CoreError::Invalid(
                "anchor crosses realignment window".into(),
            ));
        }
    }
    let l = AlignInput {
        hashes: &lh[lr],
        lines: Some(&left),
        weights: Some(&lw),
        ..Default::default()
    };
    let r = AlignInput {
        hashes: &rh[rr],
        lines: Some(&right),
        weights: Some(&rw),
        ..Default::default()
    };
    let strategy =
        if job.rules.alignment.never_align_differences && job.strategy == AlignStrategy::Smart {
            AlignStrategy::Quick
        } else {
            job.strategy
        };
    match strategy {
        AlignStrategy::Quick => QuickAlign.align2_cancellable(l, r, &overrides, cancel),
        AlignStrategy::Smart => SmartAlign {
            options: SmartAlignOptions {
                skew_tolerance: job.rules.alignment.skew_tolerance as usize,
                case_tolerant: job.rules.comparison.importance.case_unimportant,
                ..Default::default()
            },
        }
        .align2_cancellable(l, r, &overrides, cancel),
        AlignStrategy::None => {
            cancel.check()?;
            let map = AlignMap::unaligned(l.hashes, r.hashes);
            cancel.check()?;
            Ok(map)
        }
        AlignStrategy::Patience => {
            use bc_core::diff::{DiffOp, PatienceDiff};
            let ops = PatienceDiff.diff_cancellable(l.hashes, r.hashes, cancel)?;
            let mut rows = Vec::new();
            for op in ops {
                cancel.check()?;
                match op {
                    DiffOp::Equal {
                        a_start,
                        b_start,
                        len,
                    } => rows.extend((0..len).map(|i| bc_core::align::AlignRow {
                        left: Some(a_start + i),
                        right: Some(b_start + i),
                        kind: DiffKind::Same,
                        center: None,
                    })),
                    DiffOp::Delete { a_start, len, .. } => {
                        rows.extend((0..len).map(|i| bc_core::align::AlignRow {
                            left: Some(a_start + i),
                            kind: DiffKind::LeftOnly,
                            ..Default::default()
                        }))
                    }
                    DiffOp::Insert { b_start, len, .. } => {
                        rows.extend((0..len).map(|i| bc_core::align::AlignRow {
                            right: Some(b_start + i),
                            kind: DiffKind::RightOnly,
                            ..Default::default()
                        }))
                    }
                }
            }
            let mut map = AlignMap::from_rows_cancellable(rows, cancel)?;
            for &(l, r) in overrides.pairs() {
                map.force_pair(l, r)?;
            }
            Ok(map)
        }
    }
}
pub(crate) fn classify_row(
    left: &SideSnapshot,
    right: &SideSnapshot,
    l: Option<usize>,
    r: Option<usize>,
    comparison: &CompiledComparison,
    cancel: &dyn CancelCheck,
) -> Result<CompactRow, CoreError> {
    let lt = l.and_then(|i| left.lines.line_bytes(i));
    let rt = r.and_then(|i| right.lines.line_bytes(i));
    let empty = Arc::<[bc_formats::grammar::Token]>::from([]);
    let tokens = |side: &SideSnapshot,
                  index: Option<usize>|
     -> Result<Arc<[bc_formats::grammar::Token]>, CoreError> {
        if lt == rt {
            Ok(empty.clone())
        } else if let Some(i) = index {
            side.syntax.tokens_checked(&side.lines, i, cancel, true)
        } else {
            Ok(empty.clone())
        }
    };
    let ltok = tokens(left, l)?;
    let rtok = tokens(right, r)?;
    fn view<'a>(
        bytes: &'a [u8],
        index: usize,
        side: &'a SideSnapshot,
        tokens: &'a [bc_formats::grammar::Token],
    ) -> Result<LineView<'a>, CoreError> {
        Ok(LineView {
            text: std::str::from_utf8(bytes).map_err(|e| CoreError::Invalid(e.to_string()))?,
            tokens,
            format: side.format.as_deref(),
            ending: side.lines.ending(index).unwrap(),
        })
    }
    let importance = match (lt, rt, l, r) {
        (Some(lt), Some(rt), Some(l), Some(r)) => {
            comparison.classify(view(lt, l, left, &ltok)?, view(rt, r, right, &rtok)?)
        }
        (Some(lt), None, Some(l), _) => {
            comparison.classify_orphan(view(lt, l, left, &ltok)?, Side::Left)
        }
        (None, Some(rt), _, Some(r)) => {
            comparison.classify_orphan(view(rt, r, right, &rtok)?, Side::Right)
        }
        _ => return Err(CoreError::Invalid("empty text map row".into())),
    };
    let kind = match (l, r) {
        (Some(_), Some(_)) => {
            if importance == Importance::Same {
                DiffKind::Same
            } else {
                DiffKind::Changed
            }
        }
        (Some(_), None) => DiffKind::LeftOnly,
        _ => DiffKind::RightOnly,
    };
    CompactRow::new(l, r, kind, importance)
}
fn classify_alignment(
    align: AlignMap,
    left: &SideSnapshot,
    right: &SideSnapshot,
    lo: usize,
    ro: usize,
    comparison: &CompiledComparison,
    cancel: &dyn CancelCheck,
) -> Result<Vec<CompactRow>, CoreError> {
    let rows = align.into_rows();
    let mut output = Vec::with_capacity(rows.len());
    for (i, row) in rows.into_iter().enumerate() {
        if i % 128 == 0 {
            cancel.check()?;
        }
        output.push(classify_row(
            left,
            right,
            row.left.map(|i| i + lo),
            row.right.map(|i| i + ro),
            comparison,
            cancel,
        )?);
    }
    Ok(output)
}

#[derive(Debug)]
pub struct Realign2Job {
    pub base: Align2Job,
    pub dirty_rows: Range<usize>,
    pub previous: AlignOutput,
}
impl Realign2Job {
    pub fn from_map(map: &TextMap, dirty_rows: Range<usize>) -> Self {
        Self {
            base: Align2Job::from_map(map),
            dirty_rows,
            previous: AlignOutput {
                data: map.data.borrow().clone(),
                rules: map.data.borrow().rules.clone(),
                overrides: map.data.borrow().overrides.clone(),
                recomputed_rows: 0..0,
            },
        }
    }
    pub fn execute(&self, cancel: &dyn CancelCheck) -> Result<AlignOutput, CoreError> {
        let old = &self.previous.data;
        if self.previous.rules != self.base.rules
            || self.previous.overrides != self.base.overrides
            || self.dirty_rows.start >= self.dirty_rows.end
            || self.dirty_rows.end > old.rows.len()
        {
            return self.base.execute(cancel);
        }
        let delta = (
            self.base.left.lines.len() as isize - old.left_hashes.len() as isize,
            self.base.right.lines.len() as isize - old.right_hashes.len() as isize,
        );
        let comparison = CompiledComparison::new(&self.base.rules.comparison)?;
        let unique = |row: &CompactRow| {
            if row.kind != DiffKind::Same {
                return false;
            }
            let (Some(l), Some(r)) = (row.index(Side::Left), row.index(Side::Right)) else {
                return false;
            };
            old.left_hashes
                .iter()
                .filter(|h| **h == old.left_hashes[l])
                .take(2)
                .count()
                == 1
                && old
                    .right_hashes
                    .iter()
                    .filter(|h| **h == old.right_hashes[r])
                    .take(2)
                    .count()
                    == 1
        };
        let mut before = None;
        let mut after = None;
        for i in (0..self.dirty_rows.start).rev().take(128) {
            cancel.check()?;
            if unique(&old.rows[i]) {
                before = Some(i);
                break;
            }
        }
        for i in (self.dirty_rows.end..old.rows.len()).take(128) {
            cancel.check()?;
            if unique(&old.rows[i]) {
                after = Some(i);
                break;
            }
        }
        let (Some(before), Some(after)) = (before, after) else {
            return self.base.execute(cancel);
        };
        let start = (
            old.rows[before].index(Side::Left).unwrap() + 1,
            old.rows[before].index(Side::Right).unwrap() + 1,
        );
        let old_end = (
            old.rows[after].index(Side::Left).unwrap(),
            old.rows[after].index(Side::Right).unwrap(),
        );
        let Some(le) = old_end.0.checked_add_signed(delta.0) else {
            return self.base.execute(cancel);
        };
        let Some(re) = old_end.1.checked_add_signed(delta.1) else {
            return self.base.execute(cancel);
        };
        if le < start.0
            || re < start.1
            || le >= self.base.left.lines.len()
            || re >= self.base.right.lines.len()
            || hash_line(&self.base.left, le, Side::Left, &comparison, cancel)?
                != old.left_hashes[old_end.0]
            || hash_line(&self.base.right, re, Side::Right, &comparison, cancel)?
                != old.right_hashes[old_end.1]
        {
            return self.base.execute(cancel);
        }
        let rebuild = |side: &SideSnapshot,
                       old: &[ContentHash],
                       start: usize,
                       end: usize,
                       old_end: usize,
                       which|
         -> Result<Vec<ContentHash>, CoreError> {
            let mut result = Vec::with_capacity(side.lines.len());
            result.extend_from_slice(&old[..start]);
            for i in start..end {
                cancel.check()?;
                result.push(hash_line(side, i, which, &comparison, cancel)?);
            }
            result.extend_from_slice(&old[old_end..]);
            Ok(result)
        };
        let lh = rebuild(
            &self.base.left,
            &old.left_hashes,
            start.0,
            le,
            old_end.0,
            Side::Left,
        )?;
        let rh = rebuild(
            &self.base.right,
            &old.right_hashes,
            start.1,
            re,
            old_end.1,
            Side::Right,
        )?;
        let align = match align_range(&self.base, &lh, &rh, start.0..le, start.1..re, cancel) {
            Ok(a) => a,
            Err(CoreError::Cancelled) => return Err(CoreError::Cancelled),
            Err(_) => return self.base.execute(cancel),
        };
        let middle = classify_alignment(
            align,
            &self.base.left,
            &self.base.right,
            start.0,
            start.1,
            &comparison,
            cancel,
        )?;
        let recomputed_rows = before + 1..before + 1 + middle.len();
        let mut rows = Vec::with_capacity(before + 1 + middle.len() + old.rows.len() - after);
        rows.extend_from_slice(&old.rows[..=before]);
        rows.extend(middle);
        rows.extend(
            old.rows[after..]
                .iter()
                .map(|r| r.shifted(delta.0, delta.1)),
        );
        let mut data = AlignedData {
            rows,
            sections: Vec::new(),
            left_hashes: Arc::new(lh),
            right_hashes: Arc::new(rh),
            revisions: (self.base.left.revision(), self.base.right.revision()),
            rules: self.base.rules.clone(),
            overrides: self.base.overrides.clone(),
        };
        data.rebuild_sections();
        cancel.check()?;
        Ok(AlignOutput {
            data: Arc::new(data),
            rules: self.base.rules.clone(),
            overrides: self.base.overrides.clone(),
            recomputed_rows,
        })
    }
}
impl Job for Realign2Job {
    fn name(&self) -> String {
        "Re-aligning text".into()
    }
    fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
        Ok(Box::new(self.execute(&ctx.cancel).map_err(job_error)?))
    }
}

// Three-way jobs intentionally remain outside this change.
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
    fn run(self: Box<Self>, _ctx: &JobContext) -> JobResult {
        todo!("three-way change")
    }
}
#[derive(Debug)]
pub struct Realign3Job {
    pub base: Align3Job,
    pub dirty_rows: Range<usize>,
}
impl Job for Realign3Job {
    fn name(&self) -> String {
        "Re-aligning merge".into()
    }
    fn run(self: Box<Self>, _ctx: &JobContext) -> JobResult {
        todo!("three-way change")
    }
}
