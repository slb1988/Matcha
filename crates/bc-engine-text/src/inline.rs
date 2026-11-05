//! Character-level differences inside a changed line pair (FR-T2).
//! Granularity and "isolated difference" merging rules are pending disassembly (spec §8).

use crate::importance::Importance;
use std::ops::Range;

/// A differing character span on one side of a paired line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSpan {
    /// Byte range within the line text.
    pub range: Range<usize>,
    pub importance: Importance,
}

/// Inline diff result for one row.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InlineDiff {
    pub left: Vec<InlineSpan>,
    pub right: Vec<InlineSpan>,
}

/// Options controlling inline granularity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineOptions {
    /// Merge differences separated by fewer than this many equal characters.
    pub join_gap: usize,
    /// Prefer word boundaries over raw character boundaries.
    pub word_boundaries: bool,
}

impl Default for InlineOptions {
    fn default() -> Self {
        Self { join_gap: 3, word_boundaries: true }
    }
}

/// Compute inline differences for a paired line.
pub fn inline_diff(left: &str, right: &str, options: &InlineOptions) -> InlineDiff {
    todo!("bc-engine-text::inline: character/word diff with gap joining")
}
