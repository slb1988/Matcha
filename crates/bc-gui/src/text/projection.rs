//! 紧凑的显示过滤索引：按区段存储，而非为百万行再复制一张表。
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayFilter {
    #[default]
    All,
    Differences,
    Same,
    Context,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayRow {
    Text(usize),
    Fold(Range<usize>),
}
#[derive(Debug, Clone)]
struct Run {
    source: Range<usize>,
    start: usize,
    folded: bool,
}
#[derive(Debug, Clone, Default)]
pub struct Projection {
    runs: Vec<Run>,
    len: usize,
}
impl Projection {
    pub fn build(
        total: usize,
        differences: &[Range<usize>],
        filter: DisplayFilter,
        context: usize,
        expanded: &[Range<usize>],
    ) -> Self {
        Self::build_with_options(total, differences, filter, context, expanded, false)
    }
    /// 文本视图专用：Differences/Same 过滤时为空隙补「N FILTERED LINES」分隔行。
    pub fn build_with_options(
        total: usize,
        differences: &[Range<usize>],
        filter: DisplayFilter,
        context: usize,
        expanded: &[Range<usize>],
        gap_folds: bool,
    ) -> Self {
        let mut wanted = match filter {
            DisplayFilter::All => std::iter::once(0..total).collect(),
            DisplayFilter::Differences | DisplayFilter::Same => differences.to_vec(),
            DisplayFilter::Context => differences
                .iter()
                .map(|r| r.start.saturating_sub(context)..r.end.saturating_add(context).min(total))
                .collect(),
        };
        if filter == DisplayFilter::Same {
            wanted = complement(total, &merge(wanted, total));
        }
        if filter == DisplayFilter::Context {
            wanted.extend_from_slice(expanded);
        }
        let mut result = Self::default();
        let mut end = 0;
        // 文本视图参考在 Differences/Same 过滤时也显示「N FILTERED LINES」分隔行；
        // 表格视图复用本类型但不显示分隔行。
        let gap_folds =
            gap_folds && matches!(filter, DisplayFilter::Differences | DisplayFilter::Same);
        for range in merge(wanted, total) {
            if (filter == DisplayFilter::Context || gap_folds) && range.start > end {
                result.push(end..range.start, true);
            }
            end = range.end;
            result.push(range, false);
        }
        if gap_folds && end < total {
            // 虚拟 EOF 行（最后一行）保持可见，不折叠。
            let eof = total - 1;
            if end < eof {
                result.push(end..eof, true);
            }
            result.push(eof..total, false);
        } else if filter == DisplayFilter::Context && end < total {
            result.push(end..total, true);
        }
        result
    }
    fn push(&mut self, source: Range<usize>, folded: bool) {
        if source.is_empty() {
            return;
        }
        let size = if folded { 1 } else { source.len() };
        self.runs.push(Run {
            source,
            start: self.len,
            folded,
        });
        self.len += size;
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
    pub fn row(&self, display: usize) -> Option<DisplayRow> {
        if display >= self.len {
            return None;
        }
        let run = &self.runs[self.runs.partition_point(|r| r.start <= display) - 1];
        Some(if run.folded {
            DisplayRow::Fold(run.source.clone())
        } else {
            DisplayRow::Text(run.source.start + display - run.start)
        })
    }
    /// 隐藏行跳到之后首个可见区段；折叠行跳到其分隔符。
    pub fn display_for(&self, source: usize) -> usize {
        let i = self.runs.partition_point(|r| r.source.end <= source);
        self.runs.get(i).map_or(self.len.saturating_sub(1), |r| {
            r.start
                + if r.folded {
                    0
                } else {
                    source
                        .saturating_sub(r.source.start)
                        .min(r.source.len() - 1)
                }
        })
    }
}
fn merge(mut ranges: Vec<Range<usize>>, total: usize) -> Vec<Range<usize>> {
    ranges.sort_by_key(|r| r.start);
    let mut output: Vec<Range<usize>> = Vec::new();
    for r in ranges {
        let r = r.start.min(total)..r.end.min(total);
        if r.is_empty() {
            continue;
        }
        if let Some(last) = output.last_mut().filter(|l| l.end >= r.start) {
            last.end = last.end.max(r.end);
        } else {
            output.push(r);
        }
    }
    output
}
fn complement(total: usize, ranges: &[Range<usize>]) -> Vec<Range<usize>> {
    let mut result = Vec::new();
    let mut start = 0;
    for r in ranges {
        if start < r.start {
            result.push(start..r.start);
        }
        start = r.end;
    }
    if start < total {
        result.push(start..total);
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_coalesces_and_expands_without_losing_original_numbers() {
        let p = Projection::build(
            100,
            &[10..12, 16..18, 80..81],
            DisplayFilter::Context,
            3,
            &[],
        );
        assert_eq!(p.row(0), Some(DisplayRow::Fold(0..7)));
        assert_eq!(p.row(1), Some(DisplayRow::Text(7)));
        assert_eq!(p.row(15), Some(DisplayRow::Fold(21..77)));
        let p = Projection::build(
            100,
            std::slice::from_ref(&(10..12)),
            DisplayFilter::Context,
            3,
            std::slice::from_ref(&(0..7)),
        );
        assert_eq!(p.row(0), Some(DisplayRow::Text(0)));
        assert_eq!(p.display_for(11), 11);
    }
    #[test]
    fn filters_cover_empty_all_same_and_difference_ranges() {
        assert_eq!(
            Projection::build(0, &[], DisplayFilter::All, 3, &[]).len(),
            0
        );
        let p = Projection::build_with_options(
            10,
            &[2..4, 7..8],
            DisplayFilter::Differences,
            3,
            &[],
            true,
        );
        assert_eq!(p.len(), 7);
        assert_eq!(p.row(0), Some(DisplayRow::Fold(0..2)));
        assert_eq!(p.row(1), Some(DisplayRow::Text(2)));
        assert_eq!(p.row(4), Some(DisplayRow::Text(7)));
        assert_eq!(p.row(5), Some(DisplayRow::Fold(8..9)));
        assert_eq!(p.row(6), Some(DisplayRow::Text(9)));
        let p =
            Projection::build_with_options(10, &[2..4, 7..8], DisplayFilter::Same, 3, &[], true);
        assert_eq!(p.len(), 9);
        assert_eq!(p.row(2), Some(DisplayRow::Fold(2..4)));
        assert_eq!(p.row(3), Some(DisplayRow::Text(4)));
        // 默认 build 不补分隔行（表格视图复用此类型）。
        let p = Projection::build(10, &[2..4, 7..8], DisplayFilter::Differences, 3, &[]);
        assert_eq!(p.len(), 3);
        assert_eq!(p.row(2), Some(DisplayRow::Text(7)));
        assert_eq!(
            Projection::build(10, &[], DisplayFilter::Context, 3, &[]).row(0),
            Some(DisplayRow::Fold(0..10))
        );
    }
}
