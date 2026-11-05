//! Alignment – BC `TBcAlign` hierarchy.
//!
//! * `TBcQuickAlign` – pure LCS alignment ([`QuickAlign`]).
//! * `TBcSmartAlign` – heuristic re-pairing of isolated / similar lines ([`SmartAlign`]);
//!   BC's default. Scoring rules are still being recovered from disassembly (spec §8).
//! * `TBcPatienceDiffAlign` – `TBcSmartAlign` subclass using patience diff.
//! * `TBcAlignLines` – the line-level driver (`TTextMapAlignLines`, `TTableMapAlignLines`).
//! * `TBcByteAlign` / `TBcIntAlign` – byte/int sequence variants for Hex compare.

use crate::cancel::{CancelCheck, NeverCancel};
use crate::diff::{DiffOp, MyersDiff};
use crate::hash::ContentHash;
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
    /// Borrowed text for Smart similarity; hashes-only callers retain Quick output.
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

/// `TBcQuickAlign` – LCS only.
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
                &left.hashes[l..li],
                &right.hashes[r..ri],
                l,
                r,
                &mut rows,
                cancel,
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
            &left.hashes[l..],
            &right.hashes[r..],
            l,
            r,
            &mut rows,
            cancel,
        )?;
        AlignMap::from_rows_cancellable(rows, cancel)
    }
}

fn quick_rows(
    left: &[ContentHash],
    right: &[ContentHash],
    lo: usize,
    ro: usize,
    rows: &mut Vec<AlignRow>,
    cancel: &dyn CancelCheck,
) -> Result<(), crate::CoreError> {
    let ops = MyersDiff::default().diff_cancellable(left, right, cancel)?;
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

/// Tunables for `TBcSmartAlign` (names provisional until scoring is recovered, spec §8).
#[derive(Debug, Clone)]
pub struct SmartAlignOptions {
    /// Minimum similarity (0..=1) for two changed lines to be paired on one row.
    pub pair_similarity_threshold: f32,
    /// Allow re-pairing across this many unimportant lines.
    pub skew_tolerance: usize,
    /// Treat whitespace-only differences as similar when scoring.
    pub whitespace_tolerant: bool,
    /// Treat case-only differences as similar when scoring.
    pub case_tolerant: bool,
    /// Bounded Unicode-character prefix used by similarity, never whole-file copies.
    pub max_line_chars: usize,
    /// Character LCS share; the remainder is word Jaccard.
    pub character_weight: f32,
}

impl Default for SmartAlignOptions {
    fn default() -> Self {
        Self {
            pair_similarity_threshold: 0.5,
            skew_tolerance: 8,
            whitespace_tolerant: true,
            case_tolerant: false,
            max_line_chars: 256,
            character_weight: 0.7,
        }
    }
}

/// `TBcSmartAlign` – heuristic alignment (BC default).
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

    pub fn align2_cancellable(
        &self,
        left: AlignInput<'_>,
        right: AlignInput<'_>,
        overrides: &AlignmentOverrides,
        cancel: &dyn CancelCheck,
    ) -> Result<AlignMap, crate::CoreError> {
        cancel.check()?;
        validate_smart(&left, &right, &self.options)?;
        let map = QuickAlign.align2_cancellable(left.clone(), right.clone(), overrides, cancel)?;
        if left.lines.is_none() || right.lines.is_none() {
            return Ok(map);
        }
        // Consume reverse indices before rebuilding, and compact rows in place: no
        // second multi-million-row array and no repeated tail-moving Vec::splice.
        let mut rows = map.into_rows();
        let (mut read, mut write) = (0, 0);
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
            // Bounded windows avoid an unbounded all-pairs matrix on large changes.
            for window in 0..ls.len().max(rs.len()).div_ceil(128) {
                let start = window * 128;
                let l = &ls[start.min(ls.len())..(start + 128).min(ls.len())];
                let r = &rs[start.min(rs.len())..(start + 128).min(rs.len())];
                let matched = smart_window(l, r, &left, &right, &self.options, cancel)?;
                for row in matched {
                    rows[write] = row;
                    write += 1;
                }
            }
        }
        rows.truncate(write);
        AlignMap::from_rows_cancellable(rows, cancel)
    }
}

fn validate_smart(
    left: &AlignInput<'_>,
    right: &AlignInput<'_>,
    options: &SmartAlignOptions,
) -> Result<(), crate::CoreError> {
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
    if !(0.0..=1.0).contains(&options.pair_similarity_threshold)
        || !(0.0..=1.0).contains(&options.character_weight)
        || !(1..=1024).contains(&options.max_line_chars)
    {
        return Err(crate::CoreError::Invalid(
            "invalid Smart similarity options".into(),
        ));
    }
    Ok(())
}

struct SimilarityLine {
    chars: Vec<char>,
    words: std::collections::HashSet<u64>,
}

impl SimilarityLine {
    fn new(bytes: &[u8], options: &SmartAlignOptions) -> Self {
        let text = String::from_utf8_lossy(&bytes[..bytes.len().min(options.max_line_chars * 4)]);
        let mut chars = Vec::new();
        for c in text.chars() {
            if options.whitespace_tolerant && c.is_whitespace() {
                continue;
            }
            if options.case_tolerant {
                chars.extend(c.to_lowercase());
            } else {
                chars.push(c);
            }
            if chars.len() >= options.max_line_chars {
                break;
            }
        }
        chars.truncate(options.max_line_chars);
        // Words retain whitespace boundaries even when the character metric ignores it.
        let prefix: String = text.chars().take(options.max_line_chars).collect();
        let words = prefix
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| !w.is_empty())
            .map(|word| {
                if options.case_tolerant {
                    xxhash_rust::xxh64::xxh64(word.to_lowercase().as_bytes(), 0)
                } else {
                    xxhash_rust::xxh64::xxh64(word.as_bytes(), 0)
                }
            })
            .collect();
        Self { chars, words }
    }

    fn score(&self, other: &Self, character_weight: f32) -> f32 {
        let mut row = vec![0usize; other.chars.len() + 1];
        for a in &self.chars {
            let mut diagonal = 0;
            for (j, b) in other.chars.iter().enumerate() {
                let old = row[j + 1];
                row[j + 1] = if a == b {
                    diagonal + 1
                } else {
                    row[j + 1].max(row[j])
                };
                diagonal = old;
            }
        }
        let total = self.chars.len() + other.chars.len();
        let character = if total == 0 {
            1.0
        } else {
            2.0 * row[other.chars.len()] as f32 / total as f32
        };
        let intersection = self.words.intersection(&other.words).count();
        let union = self.words.len() + other.words.len() - intersection;
        let word = if union == 0 {
            0.0
        } else {
            intersection as f32 / union as f32
        };
        character_weight * character + (1.0 - character_weight) * word
    }
}

fn smart_window(
    ls: &[usize],
    rs: &[usize],
    left: &AlignInput<'_>,
    right: &AlignInput<'_>,
    options: &SmartAlignOptions,
    cancel: &dyn CancelCheck,
) -> Result<Vec<AlignRow>, crate::CoreError> {
    cancel.check()?;
    let (ll, rl) = (left.lines.unwrap(), right.lines.unwrap());
    let ltext: Vec<_> = ls
        .iter()
        .map(|&i| SimilarityLine::new(ll.line_bytes(i).unwrap(), options))
        .collect();
    let rtext: Vec<_> = rs
        .iter()
        .map(|&i| SimilarityLine::new(rl.line_bytes(i).unwrap(), options))
        .collect();
    let cols = rs.len() + 1;
    let mut directions = vec![0u8; (ls.len() + 1) * cols];
    let mut previous = vec![0.0f32; cols];
    for (i, &li) in ls.iter().enumerate() {
        let mut current = vec![0.0f32; cols];
        for (j, &ri) in rs.iter().enumerate() {
            cancel.check()?;
            let (mut score, mut direction) = if previous[j + 1] > current[j] {
                (previous[j + 1], 1)
            } else {
                (current[j], 2)
            };
            if i.abs_diff(j) <= options.skew_tolerance {
                let similarity = ltext[i].score(&rtext[j], options.character_weight);
                if similarity >= options.pair_similarity_threshold && similarity > 0.0 {
                    let lw = left.weights.map_or(0, |w| w[li].0);
                    let rw = right.weights.map_or(0, |w| w[ri].0);
                    let anchor = if lw == rw { f32::from(lw) * 4.0 } else { 0.0 };
                    let candidate = previous[j] + similarity + anchor;
                    if candidate >= score {
                        score = candidate;
                        direction = 3;
                    }
                }
            }
            current[j + 1] = score;
            directions[(i + 1) * cols + j + 1] = direction;
        }
        previous = current;
    }
    let (mut i, mut j) = (ls.len(), rs.len());
    let mut rows = Vec::with_capacity(i + j);
    while i > 0 || j > 0 {
        let direction = if i == 0 {
            2
        } else if j == 0 {
            1
        } else {
            directions[i * cols + j]
        };
        match direction {
            3 => {
                i -= 1;
                j -= 1;
                rows.push(AlignRow {
                    left: Some(ls[i]),
                    right: Some(rs[j]),
                    center: None,
                    kind: DiffKind::Changed,
                });
            }
            1 => {
                i -= 1;
                rows.push(left_only(ls[i]));
            }
            _ => {
                j -= 1;
                rows.push(right_only(rs[j]));
            }
        }
    }
    rows.reverse();
    Ok(rows)
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
