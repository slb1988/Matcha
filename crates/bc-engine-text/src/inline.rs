//! Bounded Unicode character diff, gap coalescing and conservative word snapping.
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
    /// Coalesce islands separated by at most this many equal Unicode characters.
    pub join_gap: usize,
    pub word_boundaries: bool,
    pub max_line_chars: usize,
}
impl Default for InlineOptions {
    fn default() -> Self {
        Self {
            join_gap: 3,
            word_boundaries: true,
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
    for op in (MyersDiff {
        block_fallback_threshold: 0,
    })
    .diff(&lh, &rh)
    {
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
            next.left.start - p.left.end <= options.join_gap
                && next.right.start - p.right.end <= options.join_gap
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
        let importance = if overall == Importance::Unimportant
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
