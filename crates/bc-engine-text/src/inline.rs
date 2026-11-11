//! Bounded Unicode character diff with reference-style island joining.
//!
//! Differences are character-level (no word snapping by default). Two difference islands
//! are joined only when they touch, or when exactly one non-whitespace equal character
//! separates them and the same side differs on both sides of it (`a1b2c`/`x1y2c` →
//! `{a1b}2c`; `aa X Y bb` and `worl{d}`/`wor{d}l` stay separate).
use crate::importance::{CompiledComparison, Importance, LineView};
use bc_core::{
    diff::{DiffOp, Differ, MyersDiff},
    hash::ContentHash,
    Side,
};
use std::ops::Range;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineSpan {
    pub range: Range<usize>,
    pub importance: Importance,
}
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InlineDiff {
    pub left: Vec<InlineSpan>,
    pub right: Vec<InlineSpan>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineOptions {
    /// Join islands separated by at most this many equal non-whitespace characters, when
    /// the same side differs on both sides of the gap.
    pub join_gap: usize,
    /// Widen all-word islands to whole words (off by default; the reference marks
    /// characters, e.g. `{g}etValue`).
    pub word_boundaries: bool,
    pub max_line_chars: usize,
}
impl Default for InlineOptions {
    fn default() -> Self {
        Self {
            join_gap: 1,
            word_boundaries: false,
            max_line_chars: 10_000,
        }
    }
}
struct Island {
    left: Range<usize>,
    right: Range<usize>,
    joined: bool,
}
fn word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
fn snap(chars: &[char], mut range: Range<usize>) -> Range<usize> {
    if range.is_empty() || !chars[range.clone()].iter().all(|&c| word(c)) {
        return range;
    }
    while range.start > 0 && word(chars[range.start - 1]) {
        range.start -= 1;
    }
    while range.end < chars.len() && word(chars[range.end]) {
        range.end += 1;
    }
    range
}
fn push(spans: &mut Vec<InlineSpan>, span: InlineSpan) {
    if span.range.is_empty() {
        return;
    }
    if let Some(last) = spans
        .last_mut()
        .filter(|last| last.range.end >= span.range.start && last.importance == span.importance)
    {
        last.range.end = last.range.end.max(span.range.end);
    } else {
        spans.push(span);
    }
}
/// Character diff whose ties between equally long matches resolve from the line end,
/// as the reference does (`world`/`wordl` keeps the `l`; `heasdlo`/`hello` keeps the
/// second `l`): Myers on the reversed sequences, mapped back in forward order.
fn suffix_first_diff(left: &[ContentHash], right: &[ContentHash]) -> Vec<DiffOp> {
    let reversed = |side: &[ContentHash]| side.iter().rev().copied().collect::<Vec<_>>();
    let (n, m) = (left.len(), right.len());
    let mut ops: Vec<DiffOp> = (MyersDiff {
        block_fallback_threshold: 0,
    })
    .diff(&reversed(left), &reversed(right))
    .into_iter()
    .map(|op| match op {
        DiffOp::Equal {
            a_start,
            b_start,
            len,
        } => DiffOp::Equal {
            a_start: n - a_start - len,
            b_start: m - b_start - len,
            len,
        },
        DiffOp::Delete {
            a_start,
            len,
            b_pos,
        } => DiffOp::Delete {
            a_start: n - a_start - len,
            len,
            b_pos: m - b_pos,
        },
        DiffOp::Insert {
            b_start,
            len,
            a_pos,
        } => DiffOp::Insert {
            b_start: m - b_start - len,
            len,
            a_pos: n - a_pos,
        },
    })
    .collect();
    ops.reverse();
    ops
}
pub fn inline_diff(left: &str, right: &str, options: &InlineOptions) -> InlineDiff {
    let comparison = CompiledComparison::new(&Default::default()).expect("empty rules");
    inline_diff_classified(
        LineView::plain(left),
        LineView::plain(right),
        &comparison,
        options,
    )
}
pub fn inline_diff_classified(
    left: LineView<'_>,
    right: LineView<'_>,
    comparison: &CompiledComparison,
    options: &InlineOptions,
) -> InlineDiff {
    let overall = comparison.classify(left, right);
    if overall == Importance::Same {
        return InlineDiff::default();
    }
    if left
        .text
        .chars()
        .take(options.max_line_chars.saturating_add(1))
        .count()
        > options.max_line_chars
        || right
            .text
            .chars()
            .take(options.max_line_chars.saturating_add(1))
            .count()
            > options.max_line_chars
    {
        let mut result = InlineDiff::default();
        push(
            &mut result.left,
            InlineSpan {
                range: 0..left.text.len(),
                importance: overall,
            },
        );
        push(
            &mut result.right,
            InlineSpan {
                range: 0..right.text.len(),
                importance: overall,
            },
        );
        return result;
    }
    let l: Vec<_> = left.text.chars().collect();
    let r: Vec<_> = right.text.chars().collect();
    let lh: Vec<_> = l.iter().map(|&c| ContentHash(c as u64)).collect();
    let rh: Vec<_> = r.iter().map(|&c| ContentHash(c as u64)).collect();
    let lb: Vec<_> = left
        .text
        .char_indices()
        .map(|(i, _)| i)
        .chain([left.text.len()])
        .collect();
    let rb: Vec<_> = right
        .text
        .char_indices()
        .map(|(i, _)| i)
        .chain([right.text.len()])
        .collect();
    let mut islands: Vec<Island> = Vec::new();
    for op in suffix_first_diff(&lh, &rh) {
        let next = match op {
            DiffOp::Equal { .. } => continue,
            DiffOp::Delete {
                a_start,
                len,
                b_pos,
            } => Island {
                left: a_start..a_start + len,
                right: b_pos..b_pos,
                joined: false,
            },
            DiffOp::Insert {
                b_start,
                len,
                a_pos,
            } => Island {
                left: a_pos..a_pos,
                right: b_start..b_start + len,
                joined: false,
            },
        };
        if let Some(previous) = islands.last_mut().filter(|p| {
            // The equal run between islands has the same length on both sides.
            let gap = next.left.start - p.left.end;
            let flanked = (!p.left.is_empty() && !next.left.is_empty())
                || (!p.right.is_empty() && !next.right.is_empty());
            gap == 0
                || (gap <= options.join_gap
                    && flanked
                    && !l[p.left.end..next.left.start]
                        .iter()
                        .any(|c| c.is_whitespace()))
        }) {
            previous.joined |=
                next.left.start > previous.left.end || next.right.start > previous.right.end;
            previous.left.end = next.left.end;
            previous.right.end = next.right.end;
        } else {
            islands.push(next);
        }
    }
    let mut result = InlineDiff::default();
    for mut island in islands {
        if options.word_boundaries && !island.joined {
            island.left = snap(&l, island.left);
            island.right = snap(&r, island.right);
        }
        let ls = lb[island.left.start]..lb[island.left.end];
        let rs = rb[island.right.start]..rb[island.right.end];
        let case_importance =
            comparison.fragment_case_importance(left, ls.clone(), right, rs.clone());
        if case_importance == Some(Importance::Same) {
            continue;
        }
        let importance = if overall == Importance::Unimportant
            || case_importance == Some(Importance::Unimportant)
            || (comparison.span_ignored(left, Side::Left, ls.clone())
                && comparison.span_ignored(right, Side::Right, rs.clone()))
        {
            Importance::Unimportant
        } else {
            Importance::Important
        };
        push(
            &mut result.left,
            InlineSpan {
                range: ls,
                importance,
            },
        );
        push(
            &mut result.right,
            InlineSpan {
                range: rs,
                importance,
            },
        );
    }
    result
}
