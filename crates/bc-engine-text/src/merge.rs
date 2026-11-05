//! Three-way merge – behaviour of BC `TTextMergeSession` / `TTextMergeRules` and the
//! `/automerge` `/favorleft|right` `/force` `/iu` `/reviewconflicts` switches (FR-T7).
//! Alignment is center-based (`TTextMapAlign3Job`), see `jobs::Align3Job`.

use crate::importance::Importance;
use crate::rules::TextMergeRules;
use crate::TextMap;
use bc_core::Side;

/// Where a merge row's output comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeChoice {
    /// Identical on all sides – or only center differs and rules say keep center.
    Center,
    /// Only the left changed relative to center.
    Left,
    /// Only the right changed relative to center.
    Right,
    /// Both changed identically.
    Both,
    /// Both changed differently – needs a decision.
    Conflict,
    /// User typed into the output pane.
    Manual,
}

/// Per-row merge state (one entry per [`TextMap`] row).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergeRow {
    pub choice: MergeChoice,
    /// Side the user picked for a conflict, if resolved.
    pub resolved: Option<Side>,
    pub importance: Importance,
}

/// Unresolved conflict block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub rows: std::ops::Range<usize>,
    pub importance: Importance,
}

/// Outcome of an automatic merge (drives exit codes 0/14/101).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoMergeOutcome {
    pub conflicts: Vec<Conflict>,
    /// Output text produced (with CVS markers when `force_conflict_markers`).
    pub output: Vec<u8>,
    /// `true` when output was not written because conflicts remain (exit 101).
    pub withheld: bool,
}

/// Merge model over a 3-pane [`TextMap`].
#[derive(Debug)]
pub struct MergeState {
    pub rows: Vec<MergeRow>,
    pub rules: TextMergeRules,
}

impl MergeState {
    /// Initial classification of every row after a 3-way alignment.
    pub fn classify(map: &TextMap, rules: &TextMergeRules) -> Self {
        todo!("bc-engine-text::merge: classify rows (left/right/both/conflict, favor rules)")
    }

    /// Unresolved conflicts, optionally ignoring unimportant ones (`/iu`).
    pub fn conflicts(&self, ignore_unimportant: bool) -> Vec<Conflict> {
        todo!("bc-engine-text::merge: collect conflicts")
    }

    /// Resolve a conflict block by taking `side`.
    pub fn resolve(&mut self, conflict: &Conflict, side: Side) {
        todo!("bc-engine-text::merge: resolve")
    }

    /// Produce the output pane text.
    pub fn render_output(&self, map: &TextMap) -> Vec<u8> {
        todo!("bc-engine-text::merge: render output (markers when forced)")
    }
}

/// Non-interactive merge as performed by `/automerge`.
pub fn auto_merge(map: &TextMap, rules: &TextMergeRules) -> AutoMergeOutcome {
    todo!("bc-engine-text::merge: automerge")
}
