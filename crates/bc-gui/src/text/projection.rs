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
    /// Folded runs before this one (prefix count for row geometry).
    folds_before: usize,
}
#[derive(Debug, Clone, Default)]
pub struct Projection {
    runs: Vec<Run>,
    len: usize,
    folds: usize,
    /// Gap folds ("N FILTERED LINES") are drawn as thin rows; every other
    /// projection keeps uniform row heights.
    thin_folds: bool,
}
/// Row heights of a projection: text rows and (thin) fold rows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RowGeometry {
    pub row: f32,
    pub fold: f32,
}
impl RowGeometry {
    /// Reference: 15 px text rows, 9 px filtered-lines separators.
    pub fn for_row_height(row: f32) -> Self {
        Self {
            row,
            fold: (row * 9.0 / 15.0).round(),
        }
    }
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
        result.thin_folds = gap_folds;
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
            folds_before: self.folds,
        });
        self.len += size;
        self.folds += usize::from(folded);
    }
    /// Folded display rows strictly before `display`.
    fn folds_before(&self, display: usize) -> usize {
        let i = self.runs.partition_point(|r| r.start < display);
        match i.checked_sub(1).map(|i| &self.runs[i]) {
            Some(run) => run.folds_before + usize::from(run.folded),
            None => 0,
        }
    }
    pub fn has_thin_folds(&self) -> bool {
        self.thin_folds && self.folds > 0
    }
    /// Height of one display row.
    pub fn row_height(&self, display: usize, g: RowGeometry) -> f32 {
        if self.thin_folds && matches!(self.row(display), Some(DisplayRow::Fold(_))) {
            g.fold
        } else {
            g.row
        }
    }
    /// Content offset of the top of `display` (clamped to the end).
    pub fn y_of(&self, display: usize, g: RowGeometry) -> f32 {
        let display = display.min(self.len);
        if !self.thin_folds {
            return display as f32 * g.row;
        }
        display as f32 * g.row - self.folds_before(display) as f32 * (g.row - g.fold)
    }
    /// Total content height.
    pub fn height(&self, g: RowGeometry) -> f32 {
        self.y_of(self.len, g)
    }
    /// Display row covering content offset `y` (past the end: `len`).
    pub fn display_at(&self, y: f32, g: RowGeometry) -> usize {
        if !self.thin_folds {
            return (y / g.row).floor().max(0.0) as usize;
        }
        if y < 0.0 {
            return 0;
        }
        let i = self
            .runs
            .partition_point(|r| self.y_of(r.start, g) <= y)
            .saturating_sub(1);
        let Some(run) = self.runs.get(i) else {
            return 0;
        };
        let top = self.y_of(run.start, g);
        if run.folded {
            if y < top + g.fold {
                run.start
            } else {
                run.start + 1
            }
        } else {
            let size = run.source.len();
            (run.start + ((y - top) / g.row).floor() as usize).min(run.start + size)
        }
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
    #[test]
    fn thin_gap_folds_map_offsets_both_ways() {
        let g = super::RowGeometry::for_row_height(15.0);
        assert_eq!(g.fold, 9.0);
        // 10 rows, differences at 2..4 and 7..8: Fold(0..2) 2 3 Fold(4..7) 7 Fold(8..9) 9.
        let p = Projection::build_with_options(
            10,
            &[2..4, 7..8],
            DisplayFilter::Differences,
            3,
            &[],
            true,
        );
        assert_eq!(p.len(), 7);
        let tops: Vec<f32> = (0..=7).map(|d| p.y_of(d, g)).collect();
        assert_eq!(tops, [0.0, 9.0, 24.0, 39.0, 48.0, 63.0, 72.0, 87.0]);
        assert_eq!(p.height(g), 87.0);
        for d in 0..7 {
            let top = tops[d];
            let h = p.row_height(d, g);
            assert_eq!(h, tops[d + 1] - top);
            assert_eq!(p.display_at(top, g), d);
            assert_eq!(p.display_at(top + h - 0.5, g), d);
        }
        assert_eq!(p.display_at(87.0, g), 7);
        // Uniform projections (All, Context, table) keep row * index.
        let all = Projection::build(10, &[2..4, 8..9], DisplayFilter::Context, 1, &[]);
        assert_eq!(all.y_of(3, g), 45.0);
        assert_eq!(all.display_at(44.9, g), 2);
    }
}
