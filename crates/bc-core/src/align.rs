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

/// Reconcile two 2-way alignments that share the center into one 3-column map
/// (`TTextMapAlign3Job`): `a` aligns left (its left side) with center (its right side),
/// `b` aligns center (its left side) with right (its right side).
///
/// Every center line yields one row carrying its partners from `a` and `b`. Lines without
/// a center partner go into the bucket numbered by how many center lines precede them in
/// their own 2-way row order; inside a bucket, left and right orphans sit side by side by
/// position, and the bucket precedes the center line of the same number.
///
/// Guarantees: each side is an ordered partition; left↔center pairs are exactly those of
/// `a`, center↔right pairs exactly those of `b`. `kind` is `Same` only when the center
/// line is unchanged on both sides, `LeftOnly`/`RightOnly` for single-side rows, and
/// `Changed` otherwise.
pub fn reconcile3(a: &AlignMap, b: &AlignMap) -> Result<AlignMap, crate::CoreError> {
    reconcile3_cancellable(a, b, &NeverCancel)
}

pub fn reconcile3_cancellable(
    a: &AlignMap,
    b: &AlignMap,
    cancel: &dyn CancelCheck,
) -> Result<AlignMap, crate::CoreError> {
    let centers = a.right_to_row.len();
    if centers != b.left_to_row.len() {
        return Err(crate::CoreError::Invalid(
            "three-way reconcile: the two alignments disagree on the center length".into(),
        ));
    }
    // Per center line: partner and whether that pair is unchanged. Per bucket: orphans.
    let mut left_of = vec![(None, false); centers];
    let mut right_of = vec![(None, false); centers];
    let mut left_orphans: Vec<Vec<usize>> = vec![Vec::new(); centers + 1];
    let mut right_orphans: Vec<Vec<usize>> = vec![Vec::new(); centers + 1];
    let mut seen = 0;
    for row in &a.rows {
        match (row.left, row.right) {
            (left, Some(center)) => {
                left_of[center] = (left, row.kind == DiffKind::Same);
                seen = center + 1;
            }
            (Some(left), None) => left_orphans[seen].push(left),
            (None, None) => {}
        }
    }
    cancel.check()?;
    seen = 0;
    for row in &b.rows {
        match (row.left, row.right) {
            (Some(center), right) => {
                right_of[center] = (right, row.kind == DiffKind::Same);
                seen = center + 1;
            }
            (None, Some(right)) => right_orphans[seen].push(right),
            (None, None) => {}
        }
    }
    cancel.check()?;
    let mut rows = Vec::with_capacity(a.rows.len().max(b.rows.len()));
    for bucket in 0..=centers {
        if bucket % 4096 == 0 {
            cancel.check()?;
        }
        let (lefts, rights) = (&left_orphans[bucket], &right_orphans[bucket]);
        for i in 0..lefts.len().max(rights.len()) {
            let (left, right) = (lefts.get(i).copied(), rights.get(i).copied());
            rows.push(AlignRow {
                left,
                right,
                center: None,
                kind: match (left, right) {
                    (Some(_), None) => DiffKind::LeftOnly,
                    (None, Some(_)) => DiffKind::RightOnly,
                    _ => DiffKind::Changed,
                },
            });
        }
        if bucket < centers {
            let (left, left_same) = left_of[bucket];
            let (right, right_same) = right_of[bucket];
            rows.push(AlignRow {
                left,
                right,
                center: Some(bucket),
                kind: if left_same && right_same {
                    DiffKind::Same
                } else {
                    DiffKind::Changed
                },
            });
        }
    }
    AlignMap::from_rows_cancellable(rows, cancel)
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

    fn align3(
        &self,
        left: AlignInput<'_>,
        center: AlignInput<'_>,
        right: AlignInput<'_>,
    ) -> AlignMap {
        let a = self.align2(left, center.clone());
        let b = self.align2(center, right);
        reconcile3(&a, &b).expect("both alignments share the center")
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

    fn align3(
        &self,
        left: AlignInput<'_>,
        center: AlignInput<'_>,
        right: AlignInput<'_>,
    ) -> AlignMap {
        let a = self.align2(left, center.clone());
        let b = self.align2(center, right);
        reconcile3(&a, &b).expect("both alignments share the center")
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

    fn align3(
        &self,
        left: AlignInput<'_>,
        center: AlignInput<'_>,
        right: AlignInput<'_>,
    ) -> AlignMap {
        let a = self.align2(left, center.clone());
        let b = self.align2(center, right);
        reconcile3(&a, &b).expect("both alignments share the center")
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
/// Complete 档大区间重同步的偏移容差（字节）：参考工具 `RUNRANGE` 立即数 10。
const BYTE_RESYNC_SKEW: usize = 10;
/// 大区间重同步判定所需的最短连续相等字节数（防止随机数据里的单字节巧合把配对拉偏）。
const BYTE_RESYNC_MATCH: usize = 4;

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

///// 大文件锚窗采样阈值：任一侧超过该字节数时，只对 `hash % BYTE_SAMPLE_MOD == 0` 的锚窗建索引。
/// 采样按哈希值决定，两侧同一内容的锚窗同进同出，不破坏相位语义；小文件逐字节入库。
const BYTE_SAMPLE_THRESHOLD: usize = 1024 * 1024;
const BYTE_SAMPLE_MOD: u64 = 16;
/// 区间物化并行化门槛（两侧合计字节）：低于该值单线程，避免小文件的线程开销。
const BYTE_PARALLEL_THRESHOLD: usize = 4 * 1024 * 1024;

/// 采样后的锚窗：(位置, 哈希)，按位置升序。
fn sampled_windows(
    bytes: &[u8],
    window: usize,
    sample_mod: u64,
    cancel: &dyn CancelCheck,
) -> Result<Vec<(usize, u64)>, crate::CoreError> {
    let rolling = RollingBytes::new(bytes, window);
    let mut out = Vec::with_capacity(bytes.len() / sample_mod as usize + 1);
    let mut h = rolling.window_hash(0);
    let mut pos = 0usize;
    loop {
        if h.is_multiple_of(sample_mod) {
            out.push((pos, h));
        }
        if pos + window >= bytes.len() {
            break;
        }
        h = rolling.roll(pos, h);
        pos += 1;
        if pos.is_multiple_of(1 << 16) {
            cancel.check()?;
        }
    }
    Ok(out)
}

/// 锚窗索引：按 (混合哈希, 位置) 排序，同哈希候选是连续切片且位置升序；
/// 顶位分桶目录让查找为 O(1) 次访存（大文件千万级条目时二分查找的缓存缺失会主导耗时）。
struct WindowIndex {
    entries: Vec<(u64, usize)>,
    /// `dir[b]` = 首个桶号 ≥ b 的条目下标（长度 2^bits + 1）。
    dir: Vec<usize>,
    shift: u32,
}

impl WindowIndex {
    /// 奇数乘法是 2^64 上的双射：等值关系不变，顶位分布更均匀（滚动哈希高位偏差大）。
    fn mix(hash: u64) -> u64 {
        hash.wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }

    fn new(samples: &[(usize, u64)]) -> Self {
        let mut entries: Vec<(u64, usize)> = samples.iter().map(|&(p, h)| (Self::mix(h), p)).collect();
        entries.sort_unstable();
        let bits = (usize::BITS - entries.len().max(2).leading_zeros()).saturating_sub(1).clamp(1, 26);
        let shift = 64 - bits;
        let buckets = 1usize << bits;
        let mut dir = Vec::with_capacity(buckets + 1);
        let mut at = 0usize;
        for b in 0..buckets {
            while at < entries.len() && ((entries[at].0 >> shift) as usize) < b {
                at += 1;
            }
            dir.push(at);
        }
        dir.push(entries.len());
        Self { entries, dir, shift }
    }

    fn candidates(&self, hash: u64) -> &[(u64, usize)] {
        let key = Self::mix(hash);
        let b = (key >> self.shift) as usize;
        let bucket = &self.entries[self.dir[b]..self.dir[b + 1]];
        let lo = bucket.partition_point(|&(k, _)| k < key);
        let hi = lo + bucket[lo..].partition_point(|&(k, _)| k == key);
        &bucket[lo..hi]
    }
}

/// 最长递增子序列（按 j 严格递增；输入已按 i 升序）。
fn longest_increasing_chain(pairs: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut tails: Vec<usize> = Vec::new(); // tails[k] = 长度 k+1 链尾在 pairs 中的下标
    let mut prev: Vec<usize> = vec![usize::MAX; pairs.len()];
    for (idx, &(_, j)) in pairs.iter().enumerate() {
        // 相近文件的唯一锚点几乎单调递增：先比链尾，省去千万级二分的缓存缺失。
        let k = match tails.last() {
            Some(&t) if pairs[t].1 < j => tails.len(),
            _ => tails.partition_point(|&t| pairs[t].1 < j),
        };
        if k > 0 {
            prev[idx] = tails[k - 1];
        }
        if k == tails.len() {
            tails.push(idx);
        } else {
            tails[k] = idx;
        }
    }
    let mut chain = Vec::with_capacity(tails.len());
    let mut cur = tails.last().copied().unwrap_or(usize::MAX);
    while cur != usize::MAX {
        chain.push(pairs[cur]);
        cur = prev[cur];
    }
    chain.reverse();
    chain
}

/// 对齐计划的一段：先规划、后物化（物化可并行，区间细化是主要耗时）。
#[derive(Debug, Clone, Copy)]
enum ByteSeg {
    /// 两侧自 (l, r) 起连续 len 字节相同。
    Same { l: usize, r: usize, len: usize },
    /// 锚点间区间 `[l0, l1) × [r0, r1)`，交给 [`emit_gap`]。
    Gap { l0: usize, l1: usize, r0: usize, r1: usize },
}

impl ByteSeg {
    /// 物化工作量估计（并行分块用）。
    fn weight(&self) -> usize {
        match *self {
            Self::Same { len, .. } => len,
            Self::Gap { l0, l1, r0, r1 } => 4 * ((l1 - l0) + (r1 - r0)),
        }
    }
}

/// Fast/Complete 共享的锚块骨架。
///
/// 1. **唯一锚窗骨架**：两侧各只出现一次的锚窗配对，取按位置保序的最长链（patience 思路）。
///    偶然在远处重复出现的短片段（重编译 exe 里的常见指令序列、填充）因此不会把对齐拉到
///    文件远端——旧的纯贪心扫描曾因此把两个相近 exe 几乎全判为孤儿。
/// 2. **区间内贪心**：骨架锚点之间沿用原贪心扫描（取 ≥ 下界的最近锚点、向前扩展 same 游程），
///    候选限定在区间内；锚点间余量由 `refine` 决定孤儿段（Fast）还是字节级细化（Complete）。
/// 3. **物化**：计划按工作量切块，大文件多线程物化后按序拼接（结果与单线程逐字节一致）。
fn anchored_rows(
    left: &[u8],
    right: &[u8],
    refine: bool,
    cancel: &dyn CancelCheck,
) -> Result<Vec<AlignRow>, crate::CoreError> {
    let plan = byte_plan(left, right, cancel)?;
    materialize_plan(&plan, left, right, refine, cancel)
}

fn byte_plan(left: &[u8], right: &[u8], cancel: &dyn CancelCheck) -> Result<Vec<ByteSeg>, crate::CoreError> {
    let window = BYTE_ANCHOR_WINDOW;
    let mut plan = Vec::new();
    // 任一侧短于锚窗时无锚可寻，整体作为一个区间处理。
    if left.len() < window || right.len() < window {
        plan.push(ByteSeg::Gap { l0: 0, l1: left.len(), r0: 0, r1: right.len() });
        return Ok(plan);
    }
    let sample_mod = if left.len().max(right.len()) > BYTE_SAMPLE_THRESHOLD { BYTE_SAMPLE_MOD } else { 1 };
    // 两侧滚动哈希与排序互不依赖：并行。
    let (left_side, right_index) = std::thread::scope(|scope| {
        let right_job = scope.spawn(|| -> Result<WindowIndex, crate::CoreError> {
            Ok(WindowIndex::new(&sampled_windows(right, window, sample_mod, cancel)?))
        });
        let left_side = sampled_windows(left, window, sample_mod, cancel).map(|samples| {
            let index = WindowIndex::new(&samples);
            (samples, index)
        });
        let right_index = right_job.join().unwrap_or_else(|_| Err(crate::CoreError::Invalid("anchor index panicked".into())));
        (left_side, right_index)
    });
    let (left_samples, left_index) = left_side?;
    let right_index = right_index?;
    cancel.check()?;

    // 1. 唯一锚窗骨架。
    let mut unique: Vec<(usize, usize)> = Vec::new();
    for (n, &(i, h)) in left_samples.iter().enumerate() {
        if n.is_multiple_of(1 << 16) {
            cancel.check()?;
        }
        if let [(_, j)] = right_index.candidates(h) {
            if left_index.candidates(h).len() == 1 && left[i..i + window] == right[*j..*j + window] {
                unique.push((i, *j));
            }
        }
    }
    drop(left_index);
    let chain = longest_increasing_chain(&unique);
    drop(unique);
    cancel.check()?;

    // 2. 沿骨架规划：锚点前的区间走区间内贪心，锚点处向前后扩展 same 游程。
    let ctx = GreedyCtx { left, right, window, left_samples: &left_samples, right_index: &right_index };
    let (mut gl, mut gr) = (0usize, 0usize);
    for (n, &(i, j)) in chain.iter().enumerate() {
        if n.is_multiple_of(4096) {
            cancel.check()?;
        }
        if i < gl || j < gr {
            continue; // 已被上一锚点的 same 游程吞并
        }
        let (mut a, mut b) = (i, j);
        while a > gl && b > gr && left[a - 1] == right[b - 1] {
            a -= 1;
            b -= 1;
        }
        ctx.gap(&mut plan, gl, a, gr, b, cancel)?;
        (gl, gr) = push_same_run(&mut plan, left, right, a, b, left.len(), right.len());
    }
    ctx.gap(&mut plan, gl, left.len(), gr, right.len(), cancel)?;
    Ok(plan)
}

/// 从 (p, q) 起向前扩展 same 游程（不越过上界），记入计划并返回游程终点。
fn push_same_run(
    plan: &mut Vec<ByteSeg>,
    left: &[u8],
    right: &[u8],
    p: usize,
    q: usize,
    left_end: usize,
    right_end: usize,
) -> (usize, usize) {
    let len = left[p..left_end]
        .iter()
        .zip(&right[q..right_end])
        .take_while(|(a, b)| a == b)
        .count();
    if len > 0 {
        plan.push(ByteSeg::Same { l: p, r: q, len });
    }
    (p + len, q + len)
}

fn push_gap(plan: &mut Vec<ByteSeg>, l0: usize, l1: usize, r0: usize, r1: usize) {
    if l1 > l0 || r1 > r0 {
        plan.push(ByteSeg::Gap { l0, l1, r0, r1 });
    }
}

/// 区间内贪心扫描的共享上下文。
struct GreedyCtx<'a> {
    left: &'a [u8],
    right: &'a [u8],
    window: usize,
    left_samples: &'a [(usize, u64)],
    right_index: &'a WindowIndex,
}

impl GreedyCtx<'_> {
    /// 区间 `[l0, l1) × [r0, r1)` 内的贪心锚点扫描：左侧按位置取采样锚窗，配对右侧
    /// 区间内、≥ 保序下界的第一个相同锚窗；锚点间余量记为区间段。
    fn gap(
        &self,
        plan: &mut Vec<ByteSeg>,
        l0: usize,
        l1: usize,
        r0: usize,
        r1: usize,
        cancel: &dyn CancelCheck,
    ) -> Result<(), crate::CoreError> {
        let (left, right, window) = (self.left, self.right, self.window);
        let (mut gap_left, mut gap_right) = (l0, r0);
        if l1 - l0 >= window && r1 - r0 >= window {
            let mut k = self.left_samples.partition_point(|&(p, _)| p < l0);
            let mut steps = 0usize;
            while let Some(&(i, h)) = self.left_samples.get(k) {
                if i + window > l1 {
                    break;
                }
                steps += 1;
                if steps.is_multiple_of(4096) {
                    cancel.check()?;
                }
                let cands = self.right_index.candidates(h);
                let from = cands.partition_point(|&(_, j)| j < gap_right);
                let anchor = cands[from..]
                    .iter()
                    .map(|&(_, j)| j)
                    .take_while(|&j| j + window <= r1)
                    .find(|&j| right[j..j + window] == left[i..i + window]);
                let Some(j) = anchor else {
                    k += 1;
                    continue;
                };
                let (mut a, mut b) = (i, j);
                while a > gap_left && b > gap_right && left[a - 1] == right[b - 1] {
                    a -= 1;
                    b -= 1;
                }
                push_gap(plan, gap_left, a, gap_right, b);
                (gap_left, gap_right) = push_same_run(plan, left, right, a, b, l1, r1);
                k += self.left_samples[k..].partition_point(|&(pos, _)| pos < gap_left);
            }
        }
        push_gap(plan, gap_left, l1, gap_right, r1);
        Ok(())
    }
}

/// 计划 → 对齐行：按工作量切成连续块，大文件多线程物化后按序拼接。
fn materialize_plan(
    plan: &[ByteSeg],
    left: &[u8],
    right: &[u8],
    refine: bool,
    cancel: &dyn CancelCheck,
) -> Result<Vec<AlignRow>, crate::CoreError> {
    let run = |segs: &[ByteSeg]| -> Result<Vec<AlignRow>, crate::CoreError> {
        let mut rows = Vec::with_capacity(segs.iter().map(|s| s.weight()).sum::<usize>() / 4);
        for seg in segs {
            match *seg {
                ByteSeg::Same { l, r, len } => rows.extend((0..len).map(|k| AlignRow {
                    left: Some(l + k),
                    right: Some(r + k),
                    kind: DiffKind::Same,
                    ..Default::default()
                })),
                ByteSeg::Gap { l0, l1, r0, r1 } => emit_gap(&mut rows, left, right, l0, l1, r0, r1, refine, cancel)?,
            }
        }
        Ok(rows)
    };
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get()).min(16);
    if left.len() + right.len() < BYTE_PARALLEL_THRESHOLD || threads < 2 || plan.len() < 2 {
        return run(plan);
    }
    // 按累计工作量切 threads×4 块（块内顺序不变），线程轮询取块。
    let total: usize = plan.iter().map(|s| s.weight()).sum();
    let target = total.div_ceil(threads * 4).max(1);
    let mut chunks: Vec<&[ByteSeg]> = Vec::new();
    let (mut start, mut acc) = (0usize, 0usize);
    for (k, seg) in plan.iter().enumerate() {
        acc += seg.weight();
        if acc >= target {
            chunks.push(&plan[start..=k]);
            start = k + 1;
            acc = 0;
        }
    }
    if start < plan.len() {
        chunks.push(&plan[start..]);
    }
    let next = std::sync::atomic::AtomicUsize::new(0);
    let mut parts: Vec<Option<Result<Vec<AlignRow>, crate::CoreError>>> = (0..chunks.len()).map(|_| None).collect();
    let results = std::sync::Mutex::new(&mut parts);
    std::thread::scope(|scope| {
        for _ in 0..threads.min(chunks.len()) {
            scope.spawn(|| loop {
                let k = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let Some(chunk) = chunks.get(k) else { break };
                let out = run(chunk);
                let failed = out.is_err();
                if let Ok(mut guard) = results.lock() {
                    guard[k] = Some(out);
                }
                if failed {
                    break;
                }
            });
        }
    });
    let mut rows = Vec::with_capacity(left.len().max(right.len()));
    for part in parts {
        let part = part.ok_or_else(|| crate::CoreError::Invalid("alignment worker failed".into()))??;
        rows.extend(part);
    }
    Ok(rows)
}

/// 输出一个锚点间区间：Fast 降级为双侧孤儿段；Complete 小区间字节级细化、
/// 超限走 [`resync_gap`] 对角线重同步（参考 `RUNRANGE`，不再整段降级孤儿）。
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
    if !refine {
        push_orphans(rows, left_start, left_end, right_start, right_end);
        return Ok(());
    }
    if left_len < BYTE_REFINE_LIMIT && right_len < BYTE_REFINE_LIMIT {
        return refine_gap(rows, left, right, left_start, left_end, right_start, right_end, cancel);
    }
    resync_gap(rows, left, right, left_start, left_end, right_start, right_end, cancel)
}

/// Complete 档大区间（任一侧 ≥ 5000）– 参考工具 `RUNRANGE` 的对应物：沿对角线逐字节配对，
/// 相等为 same、不等为 difference；失配处在 ±`BYTE_RESYNC_SKEW` 字节内寻找连续
/// `BYTE_RESYNC_MATCH` 字节相等的重同步点，找到则把跳过的字节记为该侧孤儿。
/// 区间尾部余量为孤儿。线性时间，适合整段内容都不同的大文件（如两张不同的 PNG）。
#[allow(clippy::too_many_arguments)]
fn resync_gap(
    rows: &mut Vec<AlignRow>,
    left: &[u8],
    right: &[u8],
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
    cancel: &dyn CancelCheck,
) -> Result<(), crate::CoreError> {
    let k = BYTE_RESYNC_MATCH;
    let matches = |i: usize, j: usize| {
        i + k <= left_end && j + k <= right_end && left[i..i + k] == right[j..j + k]
    };
    let (mut i, mut j) = (left_start, right_start);
    let mut steps = 0usize;
    while i < left_end && j < right_end {
        steps += 1;
        if steps.is_multiple_of(4096) {
            cancel.check()?;
        }
        if left[i] == right[j] {
            rows.push(AlignRow { left: Some(i), right: Some(j), kind: DiffKind::Same, ..Default::default() });
            i += 1;
            j += 1;
            continue;
        }
        // 当前对角线本身接下来能对上就不跳（单字节不同 = difference）。
        let skip = if matches(i + 1, j + 1) {
            None
        } else {
            (1..=BYTE_RESYNC_SKEW).find_map(|d| {
                if matches(i + d, j) {
                    Some((d, 0))
                } else if matches(i, j + d) {
                    Some((0, d))
                } else {
                    None
                }
            })
        };
        match skip {
            Some((dl, dr)) => {
                push_orphans(rows, i, i + dl, j, j + dr);
                i += dl;
                j += dr;
            }
            None => {
                rows.push(AlignRow { left: Some(i), right: Some(j), kind: DiffKind::Changed, ..Default::default() });
                i += 1;
                j += 1;
            }
        }
    }
    push_orphans(rows, i, left_end, j, right_end);
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
        fn complete_pairs_large_unrelated_files_along_the_diagonal() {
            // 两段毫无公共锚点的大内容（如两张不同 PNG）：不得整段左孤儿再整段右孤儿，
            // 而是逐位配对 difference，较长一侧的尾部为孤儿。
            let left = prng_bytes(0x1111, 150_000);
            let right = prng_bytes(0x2222, 400_000);
            let map = ByteAlign.align(&left, &right, ByteAlignStrategy::Complete);
            assert_partition(&map, left.len(), right.len());
            let (same, diff, left_only, right_only) = stats(&map);
            assert!(diff > 140_000, "diff {diff}");
            assert!(left_only < 1_000, "left orphan {left_only}");
            assert_eq!(same + diff + left_only, left.len());
            assert_eq!(same + diff + right_only, right.len());
            assert_eq!(map.rows()[0].left, Some(0));
            assert_eq!(map.rows()[0].right, Some(0));
            for row in map.rows() {
                if row.kind == DiffKind::Same {
                    assert_eq!(left[row.left.unwrap()], right[row.right.unwrap()]);
                }
            }
        }

        #[test]
        fn complete_resyncs_small_shifts_inside_a_large_gap() {
            // 每 40 字节翻转一个字节 → 无 50 字节锚窗，整体是一个 ≥ 5000 的大区间；
            // 区间中部 3 字节插入应在偏移容差内重同步为 3 个右孤儿。
            let left = prng_bytes(0x3333, 20_000);
            let mut right = left.clone();
            for b in right.iter_mut().step_by(40) {
                *b ^= 0x5A;
            }
            right.splice(10_001..10_001, [0xA1, 0xB2, 0xC3]);
            let map = ByteAlign.align(&left, &right, ByteAlignStrategy::Complete);
            assert_partition(&map, left.len(), right.len());
            let (same, diff, left_only, right_only) = stats(&map);
            assert_eq!((left_only, right_only), (0, 3));
            assert_eq!(diff, 500);
            assert_eq!(same, 19_500);
            let row = map.row_of_left(19_999).unwrap();
            assert_eq!(map.rows()[row].right, Some(20_002));
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
