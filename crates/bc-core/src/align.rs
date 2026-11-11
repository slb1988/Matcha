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

fn isolated_index(side: crate::Side) -> Option<usize> {
    match side {
        crate::Side::Left => Some(0),
        crate::Side::Right => Some(1),
        _ => None,
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
    /// Lines forced to stay unaligned (BC "Isolate"), per side.
    isolated: [std::collections::BTreeSet<usize>; 2],
}

impl AlignmentOverrides {
    pub fn pairs(&self) -> &[(usize, usize)] {
        &self.pairs
    }

    /// Lines of `side` that must never share a row with the other side.
    pub fn isolated(&self, side: crate::Side) -> Option<&std::collections::BTreeSet<usize>> {
        isolated_index(side).map(|i| &self.isolated[i])
    }

    /// True when the overrides carry neither anchors nor isolated lines.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty() && self.isolated.iter().all(|s| s.is_empty())
    }

    /// Isolate `line` of `side`: it becomes an orphan row; an anchor using it is dropped.
    /// Only the two primary sides can be isolated.
    pub fn isolate(&mut self, side: crate::Side, line: usize) -> Result<(), crate::CoreError> {
        let index = isolated_index(side).ok_or_else(|| {
            crate::CoreError::Invalid("isolate currently supports two sides".into())
        })?;
        self.pairs
            .retain(|&(l, r)| if index == 0 { l != line } else { r != line });
        self.isolated[index].insert(line);
        Ok(())
    }

    /// Same overrides with the two sides exchanged (Swap Sides).
    pub fn swapped(&self) -> Self {
        let [left, right] = self.isolated.clone();
        Self {
            pairs: self.pairs.iter().map(|&(l, r)| (r, l)).collect(),
            isolated: [right, left],
        }
    }

    /// Split every row that pairs an isolated line into separate orphan rows.
    /// `offsets` translate window-relative row indices into whole-file lines.
    pub fn apply_isolation(&self, map: AlignMap, offsets: (usize, usize)) -> AlignMap {
        if self.isolated.iter().all(|s| s.is_empty()) {
            return map;
        }
        let isolated = |row: &AlignRow| match (row.left, row.right, row.center) {
            (Some(l), Some(r), None) => {
                self.isolated[0].contains(&(l + offsets.0))
                    || self.isolated[1].contains(&(r + offsets.1))
            }
            _ => false,
        };
        if !map.rows().iter().any(isolated) {
            return map;
        }
        let mut rows = Vec::with_capacity(map.rows().len() + 1);
        for row in map.into_rows() {
            if isolated(&row) {
                rows.push(left_only(row.left.unwrap()));
                rows.push(right_only(row.right.unwrap()));
            } else {
                rows.push(row);
            }
        }
        AlignMap::from_rows(rows)
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
        // An explicit anchor overrides an earlier Isolate of either line.
        self.isolated[0].remove(&left);
        self.isolated[1].remove(&right);
        Ok(())
    }

    pub fn clear(&mut self) {
        self.pairs.clear();
        self.isolated.iter_mut().for_each(|s| s.clear());
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
///
/// 算法边界经 REA 反汇编校准（macOS 5.0.4.30422 arm64，`THEXMAPALIGN.RUN`，2026-11-11）：
/// 锚窗 50 字节、滚动哈希、Complete 档区间 < 5000 字节做字节级细化、超限降级。
#[derive(Debug, Default)]
pub struct ByteAlign;

/// Hex 三档字节对齐策略 – BC `THexAlignKind = (alignNone=0, alignFast=1, alignComplete=2)`
///（RTTI 枚举表逐字证实）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ByteAlignStrategy {
    /// 锚块骨架 + 区间字节级细化（参考工具默认）。
    #[default]
    Complete,
    /// 锚块骨架，区间不做字节级细化，降级为孤儿段（面向超大文件）。
    ///
    /// 注意：参考工具的 Fast 实际把 mismatch 区间逐位对齐（RUNNONE）；本实现按设计 D1
    /// 取更保守的孤儿段，杜绝跨位移的假 same。
    Fast,
    /// 逐字节位置对齐（offset 对 offset），长度差尾部为孤儿。
    None,
}

/// 锚窗宽度（字节）：参考工具实测值 50。
const BYTE_ANCHOR_WINDOW: usize = 50;
/// Complete 档字节级细化的区间长度上限（两侧均 < 该值才细化）：参考工具实测值 5000。
const BYTE_REFINE_LIMIT: usize = 5000;

impl ByteAlign {
    pub fn align(&self, left: &[u8], right: &[u8], strategy: ByteAlignStrategy) -> AlignMap {
        self.align_cancellable(left, right, strategy, &NeverCancel)
            .expect("NeverCancel never cancels")
    }

    pub fn align_cancellable(
        &self,
        left: &[u8],
        right: &[u8],
        strategy: ByteAlignStrategy,
        cancel: &dyn CancelCheck,
    ) -> Result<AlignMap, crate::CoreError> {
        let rows = match strategy {
            ByteAlignStrategy::None => positional_rows(left, right),
            ByteAlignStrategy::Fast => anchored_rows(left, right, false, cancel)?,
            ByteAlignStrategy::Complete => anchored_rows(left, right, true, cancel)?,
        };
        AlignMap::from_rows_cancellable(rows, cancel)
    }
}

/// None 档：第 i 字节对第 i 字节，不等为 difference；长度差尾部为孤儿。
fn positional_rows(left: &[u8], right: &[u8]) -> Vec<AlignRow> {
    let n = left.len().min(right.len());
    let mut rows = Vec::with_capacity(left.len().max(right.len()));
    for i in 0..n {
        rows.push(AlignRow {
            left: Some(i),
            right: Some(i),
            kind: if left[i] == right[i] { DiffKind::Same } else { DiffKind::Changed },
            ..Default::default()
        });
    }
    rows.extend((n..left.len()).map(|i| AlignRow {
        left: Some(i),
        kind: DiffKind::LeftOnly,
        ..Default::default()
    }));
    rows.extend((n..right.len()).map(|i| AlignRow {
        right: Some(i),
        kind: DiffKind::RightOnly,
        ..Default::default()
    }));
    rows
}

/// Rabin 滚动哈希（base 257，模 2^64 自然回绕；命中后一律 memcmp 复核，碰撞只浪费一次比较）。
struct RollingBytes<'a> {
    bytes: &'a [u8],
    window: usize,
    out_factor: u64,
}

impl<'a> RollingBytes<'a> {
    const BASE: u64 = 257;

    fn new(bytes: &'a [u8], window: usize) -> Self {
        let mut out_factor = 1u64;
        for _ in 0..window {
            out_factor = out_factor.wrapping_mul(Self::BASE);
        }
        Self { bytes, window, out_factor }
    }

    fn window_hash(&self, start: usize) -> u64 {
        let mut h = 0u64;
        for &b in &self.bytes[start..start + self.window] {
            h = h.wrapping_mul(Self::BASE).wrapping_add(u64::from(b));
        }
        h
    }

    /// 返回 (起点, hash) 迭代所需的滚动推进：h' = h*B - out*B^W + in。
    fn roll(&self, start: usize, hash: u64) -> u64 {
        hash.wrapping_mul(Self::BASE)
            .wrapping_sub(u64::from(self.bytes[start]).wrapping_mul(self.out_factor))
            .wrapping_add(u64::from(self.bytes[start + self.window]))
    }
}

/// Fast/Complete 共享的锚块骨架：右侧全量锚窗哈希建索引，左侧贪心扫描取保序的最近锚点，
/// 锚点间区间由 `refine` 决定降级孤儿段（Fast）还是字节级细化（Complete）。
fn anchored_rows(
    left: &[u8],
    right: &[u8],
    refine: bool,
    cancel: &dyn CancelCheck,
) -> Result<Vec<AlignRow>, crate::CoreError> {
    let window = BYTE_ANCHOR_WINDOW;
    let mut rows: Vec<AlignRow> = Vec::with_capacity(left.len().max(right.len()));
    // 任一侧短于锚窗时无锚可寻，整体作为一个区间处理。
    if left.len() < window || right.len() < window {
        emit_gap(&mut rows, left, right, 0, left.len(), 0, right.len(), refine, cancel)?;
        return Ok(rows);
    }
    // 右侧锚窗哈希 → 升序位置表。
    let right_rolling = RollingBytes::new(right, window);
    let mut index: std::collections::HashMap<u64, Vec<usize>> = std::collections::HashMap::new();
    let mut h = right_rolling.window_hash(0);
    let mut pos = 0usize;
    loop {
        index.entry(h).or_default().push(pos);
        if pos + window >= right.len() {
            break;
        }
        h = right_rolling.roll(pos, h);
        pos += 1;
        if pos.is_multiple_of(4096) {
            cancel.check()?;
        }
    }

    let left_rolling = RollingBytes::new(left, window);
    let mut gap_left = 0usize;
    let mut gap_right = 0usize;
    let mut i = 0usize; // 左侧扫描位置
    let mut j_min = 0usize; // 锚点保序下界
    let mut lh = left_rolling.window_hash(0);
    while i + window <= left.len() {
        if i.is_multiple_of(4096) {
            cancel.check()?;
        }
        let anchor = index.get(&lh).and_then(|cands| {
            // 候选位置升序；取 ≥ j_min 且 memcmp 通过的第一个。
            cands
                .iter()
                .copied()
                .skip_while(|&j| j < j_min)
                .find(|&j| right[j..j + window] == left[i..i + window])
        });
        match anchor {
            Some(j) => {
                emit_gap(&mut rows, left, right, gap_left, i, gap_right, j, refine, cancel)?;
                // 从锚点向前扩展 same 游程（逐字节比较，可超出锚窗）。
                let (mut p, mut q) = (i, j);
                while p < left.len() && q < right.len() && left[p] == right[q] {
                    rows.push(AlignRow {
                        left: Some(p),
                        right: Some(q),
                        kind: DiffKind::Same,
                        ..Default::default()
                    });
                    p += 1;
                    q += 1;
                }
                i = p;
                j_min = q;
                gap_left = p;
                gap_right = q;
                if i + window <= left.len() {
                    lh = left_rolling.window_hash(i);
                }
            }
            None => {
                if i + 1 + window > left.len() {
                    break;
                }
                lh = left_rolling.roll(i, lh);
                i += 1;
            }
        }
    }
    emit_gap(&mut rows, left, right, gap_left, left.len(), gap_right, right.len(), refine, cancel)?;
    Ok(rows)
}

/// 输出一个锚点间区间：Fast 降级为双侧孤儿段；Complete 小区间字节级细化、超限降级。
#[allow(clippy::too_many_arguments)]
fn emit_gap(
    rows: &mut Vec<AlignRow>,
    left: &[u8],
    right: &[u8],
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
    refine: bool,
    cancel: &dyn CancelCheck,
) -> Result<(), crate::CoreError> {
    let left_len = left_end - left_start;
    let right_len = right_end - right_start;
    if left_len == 0 && right_len == 0 {
        return Ok(());
    }
    if refine && left_len < BYTE_REFINE_LIMIT && right_len < BYTE_REFINE_LIMIT {
        return refine_gap(rows, left, right, left_start, left_end, right_start, right_end, cancel);
    }
    push_orphans(rows, left_start, left_end, right_start, right_end);
    Ok(())
}

fn push_orphans(
    rows: &mut Vec<AlignRow>,
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) {
    rows.extend((left_start..left_end).map(|i| AlignRow {
        left: Some(i),
        kind: DiffKind::LeftOnly,
        ..Default::default()
    }));
    rows.extend((right_start..right_end).map(|i| AlignRow {
        right: Some(i),
        kind: DiffKind::RightOnly,
        ..Default::default()
    }));
}

/// Complete 档区间细化：字节级 Myers diff，改动块内按位置配对为 difference、余量为孤儿
///（参考工具把单侧改动配对成 difference 而非双侧孤儿，四分类统计依赖此行为）。
#[allow(clippy::too_many_arguments)]
fn refine_gap(
    rows: &mut Vec<AlignRow>,
    left: &[u8],
    right: &[u8],
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
    cancel: &dyn CancelCheck,
) -> Result<(), crate::CoreError> {
    let hash = |bytes: &[u8]| -> Vec<ContentHash> { bytes.iter().map(|&b| ContentHash(u64::from(b))).collect() };
    let a = hash(&left[left_start..left_end]);
    let b = hash(&right[right_start..right_end]);
    let ops = MyersDiff::default().diff_cancellable(&a, &b, cancel)?;
    let mut dels: Vec<usize> = Vec::new(); // 当前改动块的左侧位置
    let mut inss: Vec<usize> = Vec::new(); // 当前改动块的右侧位置
    let flush = |rows: &mut Vec<AlignRow>, dels: &mut Vec<usize>, inss: &mut Vec<usize>| {
        let paired = dels.len().min(inss.len());
        for k in 0..paired {
            rows.push(AlignRow {
                left: Some(dels[k]),
                right: Some(inss[k]),
                kind: DiffKind::Changed,
                ..Default::default()
            });
        }
        for &i in &dels[paired..] {
            rows.push(AlignRow { left: Some(i), kind: DiffKind::LeftOnly, ..Default::default() });
        }
        for &i in &inss[paired..] {
            rows.push(AlignRow { right: Some(i), kind: DiffKind::RightOnly, ..Default::default() });
        }
        dels.clear();
        inss.clear();
    };
    for op in ops {
        match op {
            DiffOp::Equal { a_start, b_start, len } => {
                flush(rows, &mut dels, &mut inss);
                rows.extend((0..len).map(|k| AlignRow {
                    left: Some(left_start + a_start + k),
                    right: Some(right_start + b_start + k),
                    kind: DiffKind::Same,
                    ..Default::default()
                }));
            }
            DiffOp::Delete { a_start, len, .. } => {
                dels.extend((0..len).map(|k| left_start + a_start + k));
            }
            DiffOp::Insert { b_start, len, .. } => {
                inss.extend((0..len).map(|k| right_start + b_start + k));
            }
        }
    }
    flush(rows, &mut dels, &mut inss);
    Ok(())
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

    mod byte_align {
        use super::*;

        /// (same, difference, left orphan, right orphan) 四分类计数。
        fn stats(map: &AlignMap) -> (usize, usize, usize, usize) {
            let (mut same, mut diff, mut left_only, mut right_only) = (0, 0, 0, 0);
            for row in map.rows() {
                match row.kind {
                    DiffKind::Same => same += 1,
                    DiffKind::Changed => diff += 1,
                    DiffKind::LeftOnly => left_only += 1,
                    DiffKind::RightOnly => right_only += 1,
                }
            }
            (same, diff, left_only, right_only)
        }

        fn assert_partition(map: &AlignMap, left_len: usize, right_len: usize) {
            // AlignMap 构造已校验分区不变量；这里再显式核对偏移覆盖与顺序。
            let left_offsets: Vec<usize> = map.rows().iter().filter_map(|r| r.left).collect();
            let right_offsets: Vec<usize> = map.rows().iter().filter_map(|r| r.right).collect();
            assert_eq!(left_offsets, (0..left_len).collect::<Vec<_>>());
            assert_eq!(right_offsets, (0..right_len).collect::<Vec<_>>());
        }

        #[test]
        fn none_pairs_positionally_and_marks_changes() {
            let left = [0u8, 1, 2, 3, 4];
            let right = [0u8, 9, 2, 3, 5];
            let map = ByteAlign.align(&left, &right, ByteAlignStrategy::None);
            assert_eq!(stats(&map), (3, 2, 0, 0));
            assert_eq!(map.rows()[1].kind, DiffKind::Changed);
            assert_eq!(map.rows()[4].kind, DiffKind::Changed);
            assert_partition(&map, 5, 5);
        }

        #[test]
        fn none_orphans_the_longer_tail() {
            // 左长：尾部 2 字节左孤儿
            let map = ByteAlign.align(&[1, 2, 3, 4, 5, 6], &[1, 2, 3, 4], ByteAlignStrategy::None);
            assert_eq!(stats(&map), (4, 0, 2, 0));
            assert_eq!(map.rows()[4], AlignRow { left: Some(4), kind: DiffKind::LeftOnly, ..Default::default() });
            assert_partition(&map, 6, 4);
            // 右长：尾部 2 字节右孤儿
            let map = ByteAlign.align(&[1, 2, 3, 4], &[1, 2, 3, 4, 5, 6], ByteAlignStrategy::None);
            assert_eq!(stats(&map), (4, 0, 0, 2));
            assert_eq!(map.rows()[5], AlignRow { right: Some(5), kind: DiffKind::RightOnly, ..Default::default() });
            assert_partition(&map, 4, 6);
        }

        #[test]
        fn none_marks_shifted_content_as_differences() {
            // 5 字节插入在 None 档下表现为后续全部错位为 difference + 尾部孤儿。
            let left: Vec<u8> = (0..32).collect();
            let mut right: Vec<u8> = (0..5).collect();
            right.extend([0xAA; 5]);
            right.extend(5..32);
            let map = ByteAlign.align(&left, &right, ByteAlignStrategy::None);
            assert_eq!(stats(&map), (5, 27, 0, 5));
            assert_partition(&map, 32, 37);
        }

        /// 仓库内已提交的参考语料（docs/reference-parity/hex-compare/corpus/，SHA-256 校验过）。
        fn corpus(name: &str) -> Vec<u8> {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/reference-parity/hex-compare/corpus/").to_owned() + name;
            std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
        }

        #[test]
        fn complete_reproduces_the_reference_corpus_stats() {
            let left = corpus("left.bin");
            let right = corpus("right.bin");
            assert_eq!((left.len(), right.len()), (4160, 4144));
            let map = ByteAlign.align(&left, &right, ByteAlignStrategy::Complete);
            // 参考语料期望（Hex Compare Info 实测）：4120 same / 8 difference / 32 left orphan / 16 right orphan。
            assert_eq!(stats(&map), (4120, 8, 32, 16));
            assert_partition(&map, 4160, 4144);
        }

        #[test]
        fn complete_keeps_pairing_after_an_odd_length_insert() {
            let left = corpus("l2.bin");
            let right = corpus("r2.bin");
            assert_eq!((left.len(), right.len()), (2048, 2053));
            let map = ByteAlign.align(&left, &right, ByteAlignStrategy::Complete);
            assert_eq!(stats(&map), (2048, 0, 0, 5));
            // 插入点（0x123）之后两侧仍逐字节配对为 same，不错位到文件尾。
            let row = map.row_of_left(0x123).unwrap();
            assert_eq!(map.rows()[row].right, Some(0x123 + 5));
            assert_eq!(map.rows()[row].kind, DiffKind::Same);
            let last = map.row_of_left(2047).unwrap();
            assert_eq!(map.rows()[last].right, Some(2052));
            assert_eq!(map.rows()[last].kind, DiffKind::Same);
            assert_partition(&map, 2048, 2053);
        }

        /// 确定性伪随机字节流（xorshift64），避免测试依赖外部文件之外的生成器。
        fn prng_bytes(seed: u64, len: usize) -> Vec<u8> {
            let mut s = seed.max(1);
            (0..len)
                .map(|_| {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    (s >> 32) as u8
                })
                .collect()
        }

        #[test]
        fn fast_preserves_invariants_and_never_invents_same() {
            // 随机语料：100 KB 基底 + 多处改动/插入/删除。
            let base = prng_bytes(0x5EED, 100 * 1024);
            let mut edited = base.clone();
            for off in [0x1000usize, 0x2104, 0x30F0] {
                edited[off] ^= 0xFF;
            }
            edited.splice(0x4000..0x4000, prng_bytes(0xA11, 997)); // 奇数长度插入
            edited.splice(0x8000..0x8040, []); // 64 字节删除
            for strategy in [ByteAlignStrategy::Fast, ByteAlignStrategy::Complete] {
                let map = ByteAlign.align(&base, &edited, strategy);
                assert_partition(&map, base.len(), edited.len());
                // same 行必须两侧字节相等（不允许把内容不同的字节报为 same）。
                for row in map.rows() {
                    if row.kind == DiffKind::Same {
                        assert_eq!(base[row.left.unwrap()], edited[row.right.unwrap()]);
                    }
                }
            }
            // Fast 的 same 计数不得超过 Complete（同一锚块骨架下 Complete 只会更精细）。
            let fast = stats(&ByteAlign.align(&base, &edited, ByteAlignStrategy::Fast));
            let complete = stats(&ByteAlign.align(&base, &edited, ByteAlignStrategy::Complete));
            assert!(fast.0 <= complete.0, "fast {fast:?} vs complete {complete:?}");
        }
    }
}
