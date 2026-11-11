//! Alignment – BC `TBcAlign` hierarchy.
//!
//! * `TBcQuickAlign` – pure LCS alignment ([`QuickAlign`]).
//! * `TBcSmartAlign` – Myers skeleton plus difference-block resolution ([`SmartAlign`]);
//!   BC's default ("Standard"). Blocks are left orphaned, paired by position, or paired
//!   by the closeness DP depending on the session switches (see [`SmartAlignOptions`]).
//! * `TBcPatienceDiffAlign` – the same block resolution on a patience skeleton
//!   ([`PatienceAlign`]).
//! * `TBcAlignLines` – the line-level driver (`TTextMapAlignLines`, `TTableMapAlignLines`).
//! * `TBcByteAlign` / `TBcIntAlign` – byte/int sequence variants for Hex compare.

use crate::cancel::{CancelCheck, NeverCancel};
use crate::diff::{DiffOp, MyersDiff, PatienceDiff};
use crate::hash::{ContentHash, U64KeyBuild};
use crate::DiffKind;

/// Alignment strategy selector exposed in the text session "Alignment" frame
/// (`TTextSessionAlignmentFrame`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignStrategy {
    /// `TBcQuickAlign`
    Quick,
    /// `TBcSmartAlign` (default)
    #[default]
    Smart,
    /// `TBcPatienceDiffAlign`
    Patience,
    /// No alignment: line *n* ↔ line *n* ("Never align" session option).
    None,
}

/// One row of an aligned view. Up to three inputs (3-way merge aligns around the center).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AlignRow {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub center: Option<usize>,
    pub kind: DiffKind,
}

/// Row-oriented mapping between source line indices – the output of an [`Aligner`].
///
/// `AlignMap` is what views scroll through: each row shows the paired lines (or a gap).
#[derive(Debug, Clone, Default)]
pub struct AlignMap {
    rows: Vec<AlignRow>,
    left_to_row: Vec<usize>,
    right_to_row: Vec<usize>,
    center_to_row: Vec<usize>,
    difference_starts: Vec<usize>,
}

impl AlignMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from rows, computing the reverse indices.
    pub fn from_rows(rows: Vec<AlignRow>) -> Self {
        Self::try_from_rows(rows).expect("alignment rows must partition each side in order")
    }

    /// Checked constructor for externally supplied rows. Each present side must
    /// start at zero and contain every index exactly once; empty rows are invalid.
    pub fn try_from_rows(rows: Vec<AlignRow>) -> Result<Self, crate::CoreError> {
        Self::from_rows_cancellable(rows, &NeverCancel)
    }

    pub fn from_rows_cancellable(
        rows: Vec<AlignRow>,
        cancel: &dyn CancelCheck,
    ) -> Result<Self, crate::CoreError> {
        cancel.check()?;
        let mut result = Self::new();
        let mut in_difference = false;
        for (row_index, row) in rows.iter().enumerate() {
            if row_index % 4096 == 0 {
                cancel.check()?;
            }
            if row.left.is_none() && row.right.is_none() && row.center.is_none() {
                return Err(crate::CoreError::Invalid("empty alignment row".into()));
            }
            for (line, reverse) in [
                (row.left, &mut result.left_to_row),
                (row.right, &mut result.right_to_row),
                (row.center, &mut result.center_to_row),
            ] {
                if let Some(line) = line {
                    if line != reverse.len() {
                        return Err(crate::CoreError::Invalid(
                            "alignment side is not a contiguous ordered partition".into(),
                        ));
                    }
                    reverse.push(row_index);
                }
            }
            let different = row.kind != DiffKind::Same;
            if different && !in_difference {
                result.difference_starts.push(row_index);
            }
            in_difference = different;
        }
        cancel.check()?;
        result.rows = rows;
        Ok(result)
    }

    /// No-alignment strategy: pair nth lines and leave a longer side's tail orphaned.
    pub fn unaligned(left: &[ContentHash], right: &[ContentHash]) -> Self {
        let rows = (0..left.len().max(right.len()))
            .map(|i| match (left.get(i), right.get(i)) {
                (Some(l), Some(r)) => AlignRow {
                    left: Some(i),
                    right: Some(i),
                    center: None,
                    kind: if l == r {
                        DiffKind::Same
                    } else {
                        DiffKind::Changed
                    },
                },
                (Some(_), None) => left_only(i),
                (None, Some(_)) => right_only(i),
                (None, None) => unreachable!(),
            })
            .collect();
        Self::from_rows(rows)
    }

    /// Consume the map, releasing reverse indices before another map is built.
    pub fn into_rows(self) -> Vec<AlignRow> {
        self.rows
    }

    pub fn rows(&self) -> &[AlignRow] {
        &self.rows
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn row(&self, index: usize) -> Option<&AlignRow> {
        self.rows.get(index)
    }

    /// Row containing left line `line`.
    pub fn row_of_left(&self, line: usize) -> Option<usize> {
        self.left_to_row.get(line).copied()
    }

    /// Row containing right line `line`.
    pub fn row_of_right(&self, line: usize) -> Option<usize> {
        self.right_to_row.get(line).copied()
    }

    /// Row containing center line `line`.
    pub fn row_of_center(&self, line: usize) -> Option<usize> {
        self.center_to_row.get(line).copied()
    }

    /// First row of the next difference block, skipping the current block.
    /// None means end-of-file (or an invalid position); navigation never wraps.
    pub fn next_difference(&self, from: usize) -> Option<usize> {
        self.rows.get(from)?;
        let index = self.difference_starts.partition_point(|&row| row <= from);
        self.difference_starts.get(index).copied()
    }

    /// First row of the previous difference block, skipping the current block.
    pub fn prev_difference(&self, from: usize) -> Option<usize> {
        let row = self.rows.get(from)?;
        let mut index = self
            .difference_starts
            .partition_point(|&start| start < from);
        if row.kind != DiffKind::Same && self.difference_starts.binary_search(&from).is_err() {
            index = index.checked_sub(1)?;
        }
        self.difference_starts.get(index.checked_sub(1)?).copied()
    }

    /// Manual re-alignment: force left `l` ↔ right `r` on the same row (BC "Align With").
    pub fn force_pair(&mut self, l: usize, r: usize) -> Result<(), crate::CoreError> {
        let left_row = self
            .row_of_left(l)
            .ok_or_else(|| crate::CoreError::Invalid("left line out of bounds".into()))?;
        let right_row = self
            .row_of_right(r)
            .ok_or_else(|| crate::CoreError::Invalid("right line out of bounds".into()))?;
        if !self.center_to_row.is_empty() {
            return Err(crate::CoreError::Invalid(
                "manual pairing currently supports two sides".into(),
            ));
        }
        if left_row == right_row {
            return Ok(());
        }
        let start = left_row.min(right_row);
        let end = left_row.max(right_row);
        let mut replacement = Vec::new();
        for row in &self.rows[start..=end] {
            if let Some(left) = row.left.filter(|&i| i < l) {
                replacement.push(left_only(left));
            }
            if let Some(right) = row.right.filter(|&i| i < r) {
                replacement.push(right_only(right));
            }
        }
        replacement.push(AlignRow {
            left: Some(l),
            right: Some(r),
            center: None,
            kind: DiffKind::Changed,
        });
        for row in &self.rows[start..=end] {
            if let Some(left) = row.left.filter(|&i| i > l) {
                replacement.push(left_only(left));
            }
            if let Some(right) = row.right.filter(|&i| i > r) {
                replacement.push(right_only(right));
            }
        }
        self.rows.splice(start..=end, replacement);
        *self = Self::from_rows(std::mem::take(&mut self.rows));
        Ok(())
    }
}

fn left_only(line: usize) -> AlignRow {
    AlignRow {
        left: Some(line),
        kind: DiffKind::LeftOnly,
        ..Default::default()
    }
}

fn right_only(line: usize) -> AlignRow {
    AlignRow {
        right: Some(line),
        kind: DiffKind::RightOnly,
        ..Default::default()
    }
}

/// Session-owned, ordered manual anchors, deliberately separate from an AlignMap.
/// Reuse this value across recompare/realign; clear it to recover automatic output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AlignmentOverrides {
    pairs: Vec<(usize, usize)>,
}

impl AlignmentOverrides {
    pub fn pairs(&self) -> &[(usize, usize)] {
        &self.pairs
    }

    /// Reject duplicate-side/crossing anchors rather than violating source order.
    pub fn insert(&mut self, left: usize, right: usize) -> Result<(), crate::CoreError> {
        let pos = self.pairs.partition_point(|&(l, _)| l < left);
        if self.pairs.get(pos) == Some(&(left, right)) {
            return Ok(());
        }
        if (pos > 0 && self.pairs[pos - 1].1 >= right)
            || self
                .pairs
                .get(pos)
                .is_some_and(|&(l, r)| l == left || r <= right)
        {
            return Err(crate::CoreError::Invalid(
                "manual alignment anchors cross or reuse a side line".into(),
            ));
        }
        self.pairs.insert(pos, (left, right));
        Ok(())
    }

    pub fn clear(&mut self) {
        self.pairs.clear();
    }
}

/// Per-line weight hint fed into Smart alignment (`TTextWeightList`): lines matching a
/// format's weight regex (e.g. `^\s*object` in DFM) anchor alignment more strongly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LineWeight(pub u8);

/// Input for an aligner: hashes plus optional weights and a "unimportant" mask.
#[derive(Debug, Clone, Default)]
pub struct AlignInput<'a> {
    pub hashes: &'a [ContentHash],
    /// Borrowed text for the closeness DP; hashes-only callers keep the orphan skeleton
    /// when closeness is on.
    pub lines: Option<&'a crate::line::LineList>,
    pub weights: Option<&'a [LineWeight]>,
    /// Lines whose content is entirely unimportant (comments etc.).
    pub unimportant: Option<&'a [bool]>,
}

/// 2-way / 3-way aligner – `TBcAlign::Align`.
pub trait Aligner {
    fn strategy(&self) -> AlignStrategy;

    /// Align two sequences into rows.
    fn align2(&self, left: AlignInput<'_>, right: AlignInput<'_>) -> AlignMap;

    /// Align three sequences around the center (`TTextMapAlign3Job`).
    fn align3(
        &self,
        left: AlignInput<'_>,
        center: AlignInput<'_>,
        right: AlignInput<'_>,
    ) -> AlignMap;
}

/// Myers LCS backbone with positional replacement packing for display.
/// Use the unpaired entry points when raw delete/insert rows are required.
#[derive(Debug, Default)]
pub struct QuickAlign;

impl Aligner for QuickAlign {
    fn strategy(&self) -> AlignStrategy {
        AlignStrategy::Quick
    }

    fn align2(&self, left: AlignInput<'_>, right: AlignInput<'_>) -> AlignMap {
        self.align2_with_overrides(left, right, &AlignmentOverrides::default())
            .expect("valid alignment inputs")
    }

    #[allow(unused_variables)] // remaining scaffold entry point
    fn align3(
        &self,
        left: AlignInput<'_>,
        center: AlignInput<'_>,
        right: AlignInput<'_>,
    ) -> AlignMap {
        todo!("bc-core::align: quick 3-way alignment")
    }
}

impl QuickAlign {
    pub fn align2_with_overrides(
        &self,
        left: AlignInput<'_>,
        right: AlignInput<'_>,
        overrides: &AlignmentOverrides,
    ) -> Result<AlignMap, crate::CoreError> {
        self.align2_cancellable(left, right, overrides, &NeverCancel)
    }

    /// Run on a worker thread; checks cancellation during diff, expansion and indexing.
    pub fn align2_cancellable(
        &self,
        left: AlignInput<'_>,
        right: AlignInput<'_>,
        overrides: &AlignmentOverrides,
        cancel: &dyn CancelCheck,
    ) -> Result<AlignMap, crate::CoreError> {
        self.align2_impl(left, right, overrides, cancel, true)
    }

    pub fn align2_unpaired(&self, left: AlignInput<'_>, right: AlignInput<'_>) -> AlignMap {
        self.align2_unpaired_cancellable(left, right, &AlignmentOverrides::default(), &NeverCancel)
            .expect("valid alignment inputs")
    }

    /// Pure SES rows, used by Smart and NeverAlignDiffs; explicit anchors still win.
    pub fn align2_unpaired_cancellable(
        &self,
        left: AlignInput<'_>,
        right: AlignInput<'_>,
        overrides: &AlignmentOverrides,
        cancel: &dyn CancelCheck,
    ) -> Result<AlignMap, crate::CoreError> {
        self.align2_impl(left, right, overrides, cancel, false)
    }

    fn align2_impl(
        &self,
        left: AlignInput<'_>,
        right: AlignInput<'_>,
        overrides: &AlignmentOverrides,
        cancel: &dyn CancelCheck,
        pack: bool,
    ) -> Result<AlignMap, crate::CoreError> {
        skeleton(Backbone::Myers, &left, &right, overrides, cancel, pack)
    }
}

/// LCS backbone used to find the equal-line anchors before difference blocks are resolved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Backbone {
    Myers,
    Patience,
}

/// Anchor skeleton: equal lines become Same rows, manual anchors are honoured, and the
/// remaining lines are orphans unless `pack` requests Myers display packing (ADR 006).
fn skeleton(
    backbone: Backbone,
    left: &AlignInput<'_>,
    right: &AlignInput<'_>,
    overrides: &AlignmentOverrides,
    cancel: &dyn CancelCheck,
    pack: bool,
) -> Result<AlignMap, crate::CoreError> {
    cancel.check()?;
    let mut rows = Vec::new();
    let (mut l, mut r) = (0, 0);
    for &(li, ri) in overrides.pairs() {
        if li >= left.hashes.len() || ri >= right.hashes.len() {
            return Err(crate::CoreError::Invalid(
                "manual alignment anchor out of bounds".into(),
            ));
        }
        quick_rows(
            backbone,
            &left.hashes[l..li],
            &right.hashes[r..ri],
            (l, r),
            &mut rows,
            cancel,
            pack,
        )?;
        rows.push(AlignRow {
            left: Some(li),
            right: Some(ri),
            center: None,
            kind: if left.hashes[li] == right.hashes[ri] {
                DiffKind::Same
            } else {
                DiffKind::Changed
            },
        });
        l = li + 1;
        r = ri + 1;
    }
    quick_rows(
        backbone,
        &left.hashes[l..],
        &right.hashes[r..],
        (l, r),
        &mut rows,
        cancel,
        pack,
    )?;
    AlignMap::from_rows_cancellable(rows, cancel)
}

fn quick_rows(
    backbone: Backbone,
    left: &[ContentHash],
    right: &[ContentHash],
    (lo, ro): (usize, usize),
    rows: &mut Vec<AlignRow>,
    cancel: &dyn CancelCheck,
    pack: bool,
) -> Result<(), crate::CoreError> {
    let ops = match backbone {
        Backbone::Myers => MyersDiff::default().diff_cancellable(left, right, cancel)?,
        Backbone::Patience => PatienceDiff.diff_cancellable(left, right, cancel)?,
    };
    if pack {
        return packed_rows(&ops, rows, (lo, ro), (left, right), cancel);
    }
    let count: usize = ops
        .iter()
        .map(|op| match op {
            DiffOp::Equal { len, .. } | DiffOp::Delete { len, .. } | DiffOp::Insert { len, .. } => {
                len
            }
        })
        .sum();
    rows.reserve_exact(count);
    for op in ops {
        let len = match op {
            DiffOp::Equal { len, .. } | DiffOp::Delete { len, .. } | DiffOp::Insert { len, .. } => {
                len
            }
        };
        for start in (0..len).step_by(4096) {
            cancel.check()?;
            let range = start..(start + 4096).min(len);
            match op {
                DiffOp::Equal {
                    a_start, b_start, ..
                } => rows.extend(range.map(|i| AlignRow {
                    left: Some(lo + a_start + i),
                    right: Some(ro + b_start + i),
                    center: None,
                    kind: DiffKind::Same,
                })),
                DiffOp::Delete { a_start, .. } => {
                    rows.extend(range.map(|i| left_only(lo + a_start + i)))
                }
                DiffOp::Insert { b_start, .. } => {
                    rows.extend(range.map(|i| right_only(ro + b_start + i)))
                }
            }
        }
    }
    Ok(())
}

fn packed_rows(
    ops: &[DiffOp],
    rows: &mut Vec<AlignRow>,
    offsets: (usize, usize),
    hashes: (&[ContentHash], &[ContentHash]),
    cancel: &dyn CancelCheck,
) -> Result<(), crate::CoreError> {
    let mut count = 0;
    let (mut deleted, mut inserted) = (0usize, 0usize);
    for op in ops {
        match *op {
            DiffOp::Equal { len, .. } => {
                count += deleted.max(inserted) + len;
                deleted = 0;
                inserted = 0;
            }
            DiffOp::Delete { len, .. } => deleted += len,
            DiffOp::Insert { len, .. } => inserted += len,
        }
    }
    rows.reserve_exact(count + deleted.max(inserted));
    let mut cursor = 0;
    while cursor < ops.len() {
        cancel.check()?;
        if let DiffOp::Equal {
            a_start,
            b_start,
            len,
        } = ops[cursor]
        {
            for i in 0..len {
                if i % 4096 == 0 {
                    cancel.check()?;
                }
                rows.push(AlignRow {
                    left: Some(offsets.0 + a_start + i),
                    right: Some(offsets.1 + b_start + i),
                    center: None,
                    kind: DiffKind::Same,
                });
            }
            cursor += 1;
            continue;
        }
        let (mut left, mut right) = (0..0, 0..0);
        while cursor < ops.len() {
            match ops[cursor] {
                DiffOp::Equal { .. } => break,
                DiffOp::Delete { a_start, len, .. } => {
                    if left.is_empty() {
                        left.start = a_start;
                    }
                    left.end = a_start + len;
                }
                DiffOp::Insert { b_start, len, .. } => {
                    if right.is_empty() {
                        right.start = b_start;
                    }
                    right.end = b_start + len;
                }
            }
            cursor += 1;
        }
        for i in 0..left.len().max(right.len()) {
            if i % 4096 == 0 {
                cancel.check()?;
            }
            let l = (i < left.len()).then_some(left.start + i);
            let r = (i < right.len()).then_some(right.start + i);
            rows.push(match (l, r) {
                (Some(l), Some(r)) => AlignRow {
                    left: Some(offsets.0 + l),
                    right: Some(offsets.1 + r),
                    center: None,
                    kind: if hashes.0[l] == hashes.1[r] {
                        DiffKind::Same
                    } else {
                        DiffKind::Changed
                    },
                },
                (Some(l), None) => left_only(offsets.0 + l),
                (None, Some(r)) => right_only(offsets.1 + r),
                _ => unreachable!(),
            });
        }
    }
    Ok(())
}

/// Closeness DP penalty for each orphan line (left or right).
pub const CLOSENESS_GAP_PENALTY: i64 = -20;
/// Degeneration guard: a block with more lines than this on either side skips the DP and
/// falls back to sectioning.
pub const CLOSENESS_MAX_SIDE: usize = 5000;
/// Degeneration guard: this many repeated anchor lines in one block skips the DP.
pub const CLOSENESS_MAX_REPEATED_ANCHORS: usize = 1001;
/// Smart (Standard) emits block rows in slices of at most this many rows.
pub const SMART_EMIT_SEGMENT: usize = 200;
/// Patience emits block rows in slices of at most this many rows.
pub const PATIENCE_EMIT_SEGMENT: usize = 2000;

/// Session switches for `TBcSmartAlign` and its patience subclass.
///
/// How a difference block (orphan lines between two anchors) is resolved:
/// * `never_align_diffs` – every line stays an orphan;
/// * otherwise, `use_closeness == false` – the nth left line pairs with the nth right line
///   (no similarity check), and the longer side's tail stays orphaned;
/// * otherwise – the closeness DP pairs lines by byte-histogram similarity, unless the
///   block trips a degeneration guard ([`CLOSENESS_MAX_SIDE`],
///   [`CLOSENESS_MAX_REPEATED_ANCHORS`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartAlignOptions {
    /// Maximum number of lines examined when searching an anchor for a line.
    pub skew_tolerance: usize,
    /// "Use closeness matching".
    pub use_closeness: bool,
    /// "Never align differences".
    pub never_align_diffs: bool,
    /// Drop whitespace from the closeness text.
    pub whitespace_tolerant: bool,
    /// Fold case in the closeness text (follows the importance "case" option).
    pub case_tolerant: bool,
}

impl Default for SmartAlignOptions {
    fn default() -> Self {
        Self {
            skew_tolerance: 2000,
            use_closeness: true,
            never_align_diffs: false,
            whitespace_tolerant: true,
            case_tolerant: false,
        }
    }
}

/// `TBcSmartAlign` – Standard alignment (BC default).
#[derive(Debug, Default)]
pub struct SmartAlign {
    pub options: SmartAlignOptions,
}

impl Aligner for SmartAlign {
    fn strategy(&self) -> AlignStrategy {
        AlignStrategy::Smart
    }

    fn align2(&self, left: AlignInput<'_>, right: AlignInput<'_>) -> AlignMap {
        self.align2_with_overrides(left, right, &AlignmentOverrides::default())
            .expect("valid Smart alignment inputs")
    }

    #[allow(unused_variables)] // remaining scaffold entry point
    fn align3(
        &self,
        left: AlignInput<'_>,
        center: AlignInput<'_>,
        right: AlignInput<'_>,
    ) -> AlignMap {
        todo!("bc-core::align: smart 3-way alignment")
    }
}

impl SmartAlign {
    pub fn align2_with_overrides(
        &self,
        left: AlignInput<'_>,
        right: AlignInput<'_>,
        overrides: &AlignmentOverrides,
    ) -> Result<AlignMap, crate::CoreError> {
        self.align2_cancellable(left, right, overrides, &NeverCancel)
    }

    /// Takes the unpacked Myers skeleton (`align2_unpaired`) and resolves each
    /// difference block by the session switches.
    pub fn align2_cancellable(
        &self,
        left: AlignInput<'_>,
        right: AlignInput<'_>,
        overrides: &AlignmentOverrides,
        cancel: &dyn CancelCheck,
    ) -> Result<AlignMap, crate::CoreError> {
        resolve_aligned(
            Backbone::Myers,
            SMART_EMIT_SEGMENT,
            &self.options,
            &left,
            &right,
            overrides,
            cancel,
        )
    }
}

/// `TBcPatienceDiffAlign` – patience-diff anchors with Smart's difference-block resolution.
#[derive(Debug, Default)]
pub struct PatienceAlign {
    pub options: SmartAlignOptions,
}

impl Aligner for PatienceAlign {
    fn strategy(&self) -> AlignStrategy {
        AlignStrategy::Patience
    }

    fn align2(&self, left: AlignInput<'_>, right: AlignInput<'_>) -> AlignMap {
        self.align2_with_overrides(left, right, &AlignmentOverrides::default())
            .expect("valid Patience alignment inputs")
    }

    #[allow(unused_variables)] // remaining scaffold entry point
    fn align3(
        &self,
        left: AlignInput<'_>,
        center: AlignInput<'_>,
        right: AlignInput<'_>,
    ) -> AlignMap {
        todo!("bc-core::align: patience 3-way alignment")
    }
}

impl PatienceAlign {
    pub fn align2_with_overrides(
        &self,
        left: AlignInput<'_>,
        right: AlignInput<'_>,
        overrides: &AlignmentOverrides,
    ) -> Result<AlignMap, crate::CoreError> {
        self.align2_cancellable(left, right, overrides, &NeverCancel)
    }

    pub fn align2_cancellable(
        &self,
        left: AlignInput<'_>,
        right: AlignInput<'_>,
        overrides: &AlignmentOverrides,
        cancel: &dyn CancelCheck,
    ) -> Result<AlignMap, crate::CoreError> {
        resolve_aligned(
            Backbone::Patience,
            PATIENCE_EMIT_SEGMENT,
            &self.options,
            &left,
            &right,
            overrides,
            cancel,
        )
    }
}

fn resolve_aligned(
    backbone: Backbone,
    segment: usize,
    options: &SmartAlignOptions,
    left: &AlignInput<'_>,
    right: &AlignInput<'_>,
    overrides: &AlignmentOverrides,
    cancel: &dyn CancelCheck,
) -> Result<AlignMap, crate::CoreError> {
    cancel.check()?;
    validate_smart(left, right)?;
    let map = skeleton(backbone, left, right, overrides, cancel, false)?;
    // Closeness needs line text; hashes-only callers keep the orphan skeleton.
    let closeness_without_text =
        options.use_closeness && (left.lines.is_none() || right.lines.is_none());
    if options.never_align_diffs || closeness_without_text {
        return Ok(map);
    }
    let resolver = BlockResolver {
        left,
        right,
        options,
        segment,
        cancel,
    };
    // Consume reverse indices before rebuilding, and compact rows in place: a resolved
    // block never has more rows than its orphan skeleton, so `write <= read` holds.
    let mut rows = map.into_rows();
    let (mut read, mut write) = (0, 0);
    let mut resolved = Vec::new();
    while read < rows.len() {
        if read % 4096 == 0 {
            cancel.check()?;
        }
        if rows[read].left.is_some() && rows[read].right.is_some() {
            rows[write] = rows[read];
            read += 1;
            write += 1;
            continue;
        }
        let start = read;
        while read < rows.len() && (rows[read].left.is_none() || rows[read].right.is_none()) {
            if read % 4096 == 0 {
                cancel.check()?;
            }
            read += 1;
        }
        let ls: Vec<_> = rows[start..read].iter().filter_map(|r| r.left).collect();
        let rs: Vec<_> = rows[start..read].iter().filter_map(|r| r.right).collect();
        resolved.clear();
        resolver.block(&ls, &rs, &mut resolved)?;
        debug_assert!(resolved.len() <= read - start);
        for &row in &resolved {
            rows[write] = row;
            write += 1;
        }
    }
    rows.truncate(write);
    AlignMap::from_rows_cancellable(rows, cancel)
}

fn validate_smart(left: &AlignInput<'_>, right: &AlignInput<'_>) -> Result<(), crate::CoreError> {
    for input in [left, right] {
        if input.lines.is_some_and(|l| l.len() != input.hashes.len())
            || input.weights.is_some_and(|w| w.len() != input.hashes.len())
            || input
                .unimportant
                .is_some_and(|w| w.len() != input.hashes.len())
        {
            return Err(crate::CoreError::Invalid(
                "alignment text/weights/mask length differs from hashes".into(),
            ));
        }
    }
    Ok(())
}

/// Traceback directions of the closeness DP.
const DIR_LEFT_ORPHAN: u8 = 1;
const DIR_RIGHT_ORPHAN: u8 = 2;
const DIR_PAIR: u8 = 3;

struct BlockResolver<'a, 'b> {
    left: &'a AlignInput<'b>,
    right: &'a AlignInput<'b>,
    options: &'a SmartAlignOptions,
    segment: usize,
    cancel: &'a dyn CancelCheck,
}

impl BlockResolver<'_, '_> {
    /// One difference block: `ls` / `rs` are the ordered unmatched lines of each side.
    fn block(
        &self,
        ls: &[usize],
        rs: &[usize],
        out: &mut Vec<AlignRow>,
    ) -> Result<(), crate::CoreError> {
        if ls.is_empty() || rs.is_empty() || !self.options.use_closeness {
            return self.positional(ls, rs, out);
        }
        if self.dp_allowed(ls, rs) {
            self.closeness(ls, rs, out)
        } else {
            self.sectioned(ls, rs, out)
        }
    }

    /// Degeneration guards: oversized sides or too many repeated anchors.
    fn dp_allowed(&self, ls: &[usize], rs: &[usize]) -> bool {
        if ls.len() > CLOSENESS_MAX_SIDE || rs.len() > CLOSENESS_MAX_SIDE {
            return false;
        }
        let right: std::collections::HashSet<ContentHash, U64KeyBuild> =
            rs.iter().map(|&r| self.right.hashes[r]).collect();
        ls.iter()
            .filter(|&&l| right.contains(&self.left.hashes[l]))
            .take(CLOSENESS_MAX_REPEATED_ANCHORS)
            .count()
            < CLOSENESS_MAX_REPEATED_ANCHORS
    }

    fn pair(&self, l: usize, r: usize) -> AlignRow {
        AlignRow {
            left: Some(l),
            right: Some(r),
            center: None,
            kind: if self.left.hashes[l] == self.right.hashes[r] {
                DiffKind::Same
            } else {
                DiffKind::Changed
            },
        }
    }

    /// Pure positional pairing: nth ↔ nth, no similarity check, tail orphaned.
    fn positional(
        &self,
        ls: &[usize],
        rs: &[usize],
        out: &mut Vec<AlignRow>,
    ) -> Result<(), crate::CoreError> {
        for i in 0..ls.len().max(rs.len()) {
            if i % self.segment == 0 {
                self.cancel.check()?;
            }
            out.push(match (ls.get(i), rs.get(i)) {
                (Some(&l), Some(&r)) => self.pair(l, r),
                (Some(&l), None) => left_only(l),
                (None, Some(&r)) => right_only(r),
                (None, None) => unreachable!(),
            });
        }
        Ok(())
    }

    /// Fallback for degenerate blocks: walk the left side in order, anchoring each line
    /// on its first equal right line within `skew_tolerance` lines of the current right
    /// position, and resolve the sub-blocks between anchors as leaves.
    fn sectioned(
        &self,
        ls: &[usize],
        rs: &[usize],
        out: &mut Vec<AlignRow>,
    ) -> Result<(), crate::CoreError> {
        let mut positions: std::collections::HashMap<ContentHash, Vec<usize>, U64KeyBuild> =
            Default::default();
        for (j, &r) in rs.iter().enumerate() {
            positions.entry(self.right.hashes[r]).or_default().push(j);
        }
        let (mut li, mut ri) = (0, 0);
        let (mut seg_l, mut seg_r) = (0, 0);
        while li < ls.len() && ri < rs.len() {
            if li % 4096 == 0 {
                self.cancel.check()?;
            }
            let hit = positions.get(&self.left.hashes[ls[li]]).and_then(|p| {
                p.get(p.partition_point(|&j| j < ri))
                    .copied()
                    .filter(|&j| j - ri < self.options.skew_tolerance)
            });
            if let Some(j) = hit {
                self.leaf(&ls[seg_l..li], &rs[seg_r..j], out)?;
                out.push(self.pair(ls[li], rs[j]));
                ri = j + 1;
                seg_l = li + 1;
                seg_r = ri;
            }
            li += 1;
        }
        self.leaf(&ls[seg_l..], &rs[seg_r..], out)
    }

    /// Sub-block of a sectioned block: closeness when the guards allow, else positional.
    fn leaf(
        &self,
        ls: &[usize],
        rs: &[usize],
        out: &mut Vec<AlignRow>,
    ) -> Result<(), crate::CoreError> {
        if !ls.is_empty() && !rs.is_empty() && self.dp_allowed(ls, rs) {
            self.closeness(ls, rs, out)
        } else {
            self.positional(ls, rs, out)
        }
    }

    /// Monotone closeness DP maximizing summed pair scores plus gap penalties.
    /// Ties keep the pair (diagonal); a gap direction wins only when strictly better,
    /// and the right-orphan direction is tried before the left-orphan direction.
    fn closeness(
        &self,
        ls: &[usize],
        rs: &[usize],
        out: &mut Vec<AlignRow>,
    ) -> Result<(), crate::CoreError> {
        self.cancel.check()?;
        let features = |input: &AlignInput<'_>, side: &[usize]| {
            let lines = input.lines.expect("closeness requires line text");
            side.iter()
                .map(|&i| ClosenessLine::new(input, lines, i, self.options))
                .collect::<Vec<_>>()
        };
        let lf = features(self.left, ls);
        let rf = features(self.right, rs);
        let cols = rs.len() + 1;
        let mut directions = vec![0u8; (ls.len() + 1) * cols];
        directions[1..cols].fill(DIR_RIGHT_ORPHAN);
        let mut previous: Vec<i64> = (0..cols as i64)
            .map(|j| j * CLOSENESS_GAP_PENALTY)
            .collect();
        let mut current = vec![0i64; cols];
        let mut dense = [0u32; 256];
        for (i, a) in lf.iter().enumerate() {
            self.cancel.check()?;
            let i = i + 1;
            for &(byte, count) in &a.histogram {
                dense[usize::from(byte)] = count;
            }
            current[0] = i as i64 * CLOSENESS_GAP_PENALTY;
            directions[i * cols] = DIR_LEFT_ORPHAN;
            for (j, b) in rf.iter().enumerate() {
                let j = j + 1;
                let mut best = previous[j - 1] + a.score(b, &dense);
                let mut direction = DIR_PAIR;
                let right_gap = current[j - 1] + CLOSENESS_GAP_PENALTY;
                if right_gap > best {
                    best = right_gap;
                    direction = DIR_RIGHT_ORPHAN;
                }
                let left_gap = previous[j] + CLOSENESS_GAP_PENALTY;
                if left_gap > best {
                    best = left_gap;
                    direction = DIR_LEFT_ORPHAN;
                }
                current[j] = best;
                directions[i * cols + j] = direction;
            }
            for &(byte, _) in &a.histogram {
                dense[usize::from(byte)] = 0;
            }
            std::mem::swap(&mut previous, &mut current);
        }
        let (mut i, mut j) = (ls.len(), rs.len());
        let mut path = Vec::with_capacity(i + j);
        while i > 0 || j > 0 {
            let direction = directions[i * cols + j];
            path.push(direction);
            match direction {
                DIR_PAIR => {
                    i -= 1;
                    j -= 1;
                }
                DIR_LEFT_ORPHAN => i -= 1,
                _ => j -= 1,
            }
        }
        drop(directions);
        path.reverse();
        self.emit(&path, ls, rs, out)
    }

    /// Emit the traceback: each run of one direction is one block emission, sliced into
    /// at most `segment` rows (slicing never changes the pairing).
    fn emit(
        &self,
        path: &[u8],
        ls: &[usize],
        rs: &[usize],
        out: &mut Vec<AlignRow>,
    ) -> Result<(), crate::CoreError> {
        let (mut l, mut r) = (0, 0);
        for run in path.chunk_by(|a, b| a == b) {
            for slice in run.chunks(self.segment) {
                self.cancel.check()?;
                for &direction in slice {
                    out.push(match direction {
                        DIR_PAIR => {
                            l += 1;
                            r += 1;
                            self.pair(ls[l - 1], rs[r - 1])
                        }
                        DIR_LEFT_ORPHAN => {
                            l += 1;
                            left_only(ls[l - 1])
                        }
                        _ => {
                            r += 1;
                            right_only(rs[r - 1])
                        }
                    });
                }
            }
        }
        Ok(())
    }
}

/// Closeness features of one line, built from its normalized text.
struct ClosenessLine {
    hash: ContentHash,
    /// Sparse `(byte, count)` histogram of the normalized UTF-8 text.
    histogram: Vec<(u8, u32)>,
    /// Normalized text length in bytes.
    len: i64,
    /// Significant (normalized) character count.
    significant: i64,
    /// Raw line length in bytes.
    raw_len: usize,
    /// Format weight; equal weights add `4 * weight` (e.g. DFM `object` lines).
    weight: u8,
}

impl ClosenessLine {
    fn new(
        input: &AlignInput<'_>,
        lines: &crate::line::LineList,
        index: usize,
        options: &SmartAlignOptions,
    ) -> Self {
        let bytes = lines.line_bytes(index).unwrap_or_default();
        let mut counts = [0u32; 256];
        let (mut len, mut significant) = (0i64, 0i64);
        let mut count = |c: char| {
            let mut buffer = [0u8; 4];
            significant += 1;
            for &b in c.encode_utf8(&mut buffer).as_bytes() {
                counts[usize::from(b)] += 1;
                len += 1;
            }
        };
        for c in String::from_utf8_lossy(bytes).chars() {
            if options.whitespace_tolerant && c.is_whitespace() {
                continue;
            }
            if options.case_tolerant {
                c.to_lowercase().for_each(&mut count);
            } else {
                count(c);
            }
        }
        let histogram = (0u8..=255)
            .zip(counts)
            .filter(|&(_, count)| count > 0)
            .collect();
        Self {
            hash: input.hashes[index],
            histogram,
            len,
            significant,
            raw_len: bytes.len(),
            weight: input.weights.map_or(0, |w| w[index].0),
        }
    }

    /// Pair score against `other`; `dense` holds `self`'s histogram indexed by byte.
    fn score(&self, other: &Self, dense: &[u32; 256]) -> i64 {
        let base = if self.hash == other.hash {
            if self.significant == 0 && other.significant == 0 {
                -10
            } else {
                2 * (self.significant + other.significant)
                    - i64::from(self.raw_len != other.raw_len)
            }
        } else if self.len == 0 || other.len == 0 {
            (-4 * (self.len + other.len) - 1).clamp(-40, -11)
        } else {
            let common: i64 = other
                .histogram
                .iter()
                .map(|&(byte, count)| i64::from(dense[usize::from(byte)].min(count)))
                .sum();
            let t = 3 * common - self.len - other.len;
            if t < 1 {
                (4 * t - 1).max(-40)
            } else {
                25 * t / self.len.min(other.len)
            }
        };
        let anchor = if self.weight == other.weight {
            4 * i64::from(self.weight)
        } else {
            0
        };
        base + anchor
    }
}

/// Byte-sequence aligner for Hex compare – `TBcByteAlign` / `THexMapByteAlign`.
#[derive(Debug, Default)]
pub struct ByteAlign;

impl ByteAlign {
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn align(&self, left: &[u8], right: &[u8]) -> AlignMap {
        todo!("bc-core::align: byte alignment")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{hash::LineHasher, line::LineList};

    fn score_with(hasher: &LineHasher, a: &str, b: &str) -> i64 {
        let lines = LineList::from_bytes(format!("{a}\n{b}\n").into_bytes());
        let hashes = hasher.hash_lines(&lines);
        let input = AlignInput {
            hashes: &hashes,
            lines: Some(&lines),
            ..Default::default()
        };
        let options = SmartAlignOptions::default();
        let a = ClosenessLine::new(&input, &lines, 0, &options);
        let b = ClosenessLine::new(&input, &lines, 1, &options);
        let mut dense = [0u32; 256];
        for &(byte, count) in &a.histogram {
            dense[usize::from(byte)] = count;
        }
        a.score(&b, &dense)
    }

    fn score(a: &str, b: &str) -> i64 {
        score_with(&LineHasher::default(), a, b)
    }

    #[test]
    fn closeness_score_follows_the_histogram_formula() {
        // t = 3*common - lenL - lenR on whitespace-free text.
        assert_eq!(score("foo one", "bar 2"), -40); // t = -10 -> floor
        assert_eq!(score("foo one", "foo 1"), -5); // t = -1 -> 4t - 1
        assert_eq!(score("bar two", "bar 2"), -5);
        assert_eq!(score("bar two", "foo 1"), -29); // t = -7
        assert_eq!(score("alpha = 1", "alpha = 10"), 21); // t = 6 -> 25*6/7
        assert_eq!(score("abcdefghij", "0123456789"), -40);
    }

    #[test]
    fn closeness_score_handles_featureless_and_equal_lines() {
        assert_eq!(score("   ", "a"), -11); // -5 clamped up to -11
        assert_eq!(score("", "abc"), -13);
        assert_eq!(score("", "abcdefghijk"), -40);
        assert_eq!(score("", ""), -10);
        assert_eq!(score("xy", "xy"), 8); // 2 * (2 + 2)
        let loose = LineHasher {
            ignore_leading_whitespace: true,
            ..Default::default()
        };
        assert_eq!(score_with(&loose, "  xy", "xy"), 7); // raw lengths differ: -1
    }

    fn resolve(left: &[&str], right: &[&str], options: &SmartAlignOptions) -> Vec<AlignRow> {
        let hash = |side: &[&str]| -> Vec<ContentHash> {
            let lines = LineList::from_bytes(side.join("\n").into_bytes());
            LineHasher::default().hash_lines(&lines)
        };
        let (lt, rt) = (left.join("\n"), right.join("\n"));
        let (ll, rl) = (
            LineList::from_bytes(lt.into_bytes()),
            LineList::from_bytes(rt.into_bytes()),
        );
        let (lh, rh) = (hash(left), hash(right));
        let (li, ri) = (
            AlignInput {
                hashes: &lh,
                lines: Some(&ll),
                ..Default::default()
            },
            AlignInput {
                hashes: &rh,
                lines: Some(&rl),
                ..Default::default()
            },
        );
        let resolver = BlockResolver {
            left: &li,
            right: &ri,
            options,
            segment: SMART_EMIT_SEGMENT,
            cancel: &NeverCancel,
        };
        let mut out = Vec::new();
        let ls: Vec<_> = (0..left.len()).collect();
        let rs: Vec<_> = (0..right.len()).collect();
        resolver.block(&ls, &rs, &mut out).unwrap();
        out
    }

    #[test]
    fn repeated_anchor_blocks_fall_back_to_sectioning() {
        let shared: Vec<String> = (0..CLOSENESS_MAX_REPEATED_ANCHORS)
            .map(|i| format!("s{i}"))
            .collect();
        let left: Vec<&str> = shared.iter().map(String::as_str).collect();
        let mut right = vec!["x", "y"];
        right.extend(left.iter().copied());
        let rows = resolve(&left, &right, &SmartAlignOptions::default());
        assert_eq!(rows[0], right_only(0));
        assert_eq!(rows[1], right_only(1));
        assert!(rows[2..]
            .iter()
            .enumerate()
            .all(|(i, r)| r.left == Some(i) && r.right == Some(i + 2) && r.kind == DiffKind::Same));
        // Anchors further than the skew tolerance are not searched: positional leaf.
        let rows = resolve(
            &left,
            &right,
            &SmartAlignOptions {
                skew_tolerance: 2,
                ..Default::default()
            },
        );
        assert_eq!(rows.len(), right.len());
        assert!(rows[..left.len()]
            .iter()
            .enumerate()
            .all(|(i, r)| r.left == Some(i) && r.right == Some(i) && r.kind == DiffKind::Changed));
    }
}
