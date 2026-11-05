//! Alignment – BC `TBcAlign` hierarchy.
//!
//! * `TBcQuickAlign` – pure LCS alignment ([`QuickAlign`]).
//! * `TBcSmartAlign` – heuristic re-pairing of isolated / similar lines ([`SmartAlign`]);
//!   BC's default. Scoring rules are still being recovered from disassembly (spec §8).
//! * `TBcPatienceDiffAlign` – `TBcSmartAlign` subclass using patience diff.
//! * `TBcAlignLines` – the line-level driver (`TTextMapAlignLines`, `TTableMapAlignLines`).
//! * `TBcByteAlign` / `TBcIntAlign` – byte/int sequence variants for Hex compare.

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
}

impl AlignMap {
    pub fn new() -> Self {
        Self::default()
    }

    /// Build from rows, computing the reverse indices.
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn from_rows(rows: Vec<AlignRow>) -> Self {
        todo!("bc-core::align: build reverse indices")
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

    /// Index of the next row after `from` whose kind is not [`DiffKind::Same`]
    /// ("Next Difference" navigation).
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn next_difference(&self, from: usize) -> Option<usize> {
        todo!("bc-core::align: difference navigation")
    }

    /// Index of the previous difference row before `from`.
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn prev_difference(&self, from: usize) -> Option<usize> {
        todo!("bc-core::align: difference navigation")
    }

    /// Manual re-alignment: force left `l` ↔ right `r` on the same row (BC "Align With").
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn force_pair(&mut self, l: usize, r: usize) {
        todo!("bc-core::align: manual align")
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

    #[allow(unused_variables)] // remaining scaffold entry point
    fn align2(&self, left: AlignInput<'_>, right: AlignInput<'_>) -> AlignMap {
        todo!("bc-core::align: quick (LCS) alignment")
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
}

impl Default for SmartAlignOptions {
    fn default() -> Self {
        Self {
            pair_similarity_threshold: 0.5,
            skew_tolerance: 8,
            whitespace_tolerant: true,
            case_tolerant: false,
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

    #[allow(unused_variables)] // remaining scaffold entry point
    fn align2(&self, left: AlignInput<'_>, right: AlignInput<'_>) -> AlignMap {
        todo!("bc-core::align: smart alignment (pending disassembly of TBcSmartAlign)")
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

/// Byte-sequence aligner for Hex compare – `TBcByteAlign` / `THexMapByteAlign`.
#[derive(Debug, Default)]
pub struct ByteAlign;

impl ByteAlign {
    #[allow(unused_variables)] // remaining scaffold entry point
    pub fn align(&self, left: &[u8], right: &[u8]) -> AlignMap {
        todo!("bc-core::align: byte alignment")
    }
}
