use crate::text::projection::{DisplayFilter, DisplayRow, Projection};
use bc_core::cancel::CancelCheck;
use bc_engine_table::{
    compare::Importance,
    map::{TableAlignOutput, TableStatistics},
    Result, SheetStore,
};
use std::ops::Range;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Differences,
    Same,
    None,
}
#[derive(Default)]
pub struct Cache {
    pub total: usize,
    pub statistics: TableStatistics,
    pub summaries: [TableStatistics; 3],
    pub differences: Vec<Range<usize>>,
    pub diff_rows: Vec<usize>,
    pub edited_rows: Vec<usize>,
    pub bins: Vec<Importance>,
    pub different_columns: Vec<bool>,
    pub widths: Vec<f32>,
}
impl Cache {
    pub fn build(
        output: &TableAlignOutput,
        left: &SheetStore,
        right: &SheetStore,
        cancel: &dyn CancelCheck,
    ) -> Result<Self> {
        let data = &output.data;
        let rules = &output.rules;
        let mut cache = Self {
            total: data.row_count(),
            statistics: data.statistics(rules.layers, rules.ignore_unimportant),
            summaries: [
                bc_engine_table::rules::Layers::Values,
                bc_engine_table::rules::Layers::ValuesAndFormulas,
                bc_engine_table::rules::Layers::All,
            ]
            .map(|layer| data.statistics(layer, false)),
            bins: vec![Importance::Same; 512],
            ..Default::default()
        };
        let mut start = None;
        for i in 0..data.row_count() {
            if i.is_multiple_of(4096) {
                cancel.check()?;
            }
            let row = data.row(i, rules.layers, rules.ignore_unimportant).unwrap();
            if row.edited {
                cache.edited_rows.push(i);
            }
            if row.importance != Importance::Same {
                cache.diff_rows.push(i);
                cache.bins[i * 512 / data.row_count().max(1)] =
                    cache.bins[i * 512 / data.row_count().max(1)].max(row.importance);
                start.get_or_insert(i);
            } else if let Some(s) = start.take() {
                cache.differences.push(s..i);
            }
        }
        if let Some(s) = start {
            cache.differences.push(s..data.row_count());
        }
        for (c, column) in rules.columns.0.iter().enumerate() {
            cancel.check()?;
            cache.different_columns.push(
                data.column_diff(c).is_some_and(|d| {
                    d.importance(rules.layers, rules.ignore_unimportant) != Importance::Same
                }) || !column.changes.is_empty(),
            );
            let mut length = 1;
            for (sheet, col) in [(left, column.left), (right, column.right)] {
                if let Some(col) = col {
                    length = length.max(header_length(sheet, col));
                    for r in 0..sheet.row_count().min(128) {
                        length = length.max(
                            sheet
                                .display_at(r, col)
                                .unwrap_or_default()
                                .lines()
                                .map(|s| s.chars().take(60).count())
                                .max()
                                .unwrap_or(0),
                        );
                    }
                }
            }
            cache.widths.push(fit_width(length).min(420.0));
        }
        cache.statistics.structural_columns = rules
            .columns
            .0
            .iter()
            .filter(|c| !c.changes.is_empty())
            .count();
        Ok(cache)
    }
    pub fn fit_all(
        &mut self,
        output: &TableAlignOutput,
        left: &SheetStore,
        right: &SheetStore,
        cancel: &dyn CancelCheck,
    ) -> Result<()> {
        for (c, column) in output.rules.columns.0.iter().enumerate() {
            let mut length = 1;
            for (sheet, col) in [(left, column.left), (right, column.right)] {
                if let Some(col) = col {
                    length = length.max(header_length(sheet, col));
                    for row in 0..sheet.row_count() {
                        if row.is_multiple_of(1024) {
                            cancel.check()?;
                        }
                        length = length.max(
                            sheet
                                .display_at(row, col)
                                .unwrap_or_default()
                                .lines()
                                .map(|s| s.chars().count())
                                .max()
                                .unwrap_or(0),
                        );
                    }
                }
            }
            self.widths[c] = fit_width(length);
        }
        Ok(())
    }
    pub fn project(&self, filter: Filter) -> Projection {
        Projection::build(
            if filter == Filter::None {
                0
            } else {
                self.total
            },
            &self.differences,
            match filter {
                Filter::Differences => DisplayFilter::Differences,
                Filter::Same => DisplayFilter::Same,
                _ => DisplayFilter::All,
            },
            0,
            &[],
        )
    }
}
/// Reference column fit (tokens `tableColumnWidths` [47,33,67] for 5/3/8
/// character columns): 7 px per Consolas character plus 12 px padding and
/// grid line, never narrower than a lettered header with its marker.
fn fit_width(length: usize) -> f32 {
    (length as f32 * 7.0 + 12.0).max(33.0)
}
/// Header names count toward the fit only when the sheet has a header row
/// (otherwise the header shows a column letter).
fn header_length(sheet: &SheetStore, col: usize) -> usize {
    if bc_engine_table::TableData::headers(sheet).is_some() {
        sheet.column_name(col).chars().count()
    } else {
        1
    }
}
pub fn layer_index(layer: bc_engine_table::rules::Layers) -> usize {
    match layer {
        bc_engine_table::rules::Layers::Values => 0,
        bc_engine_table::rules::Layers::ValuesAndFormulas => 1,
        bc_engine_table::rules::Layers::All => 2,
    }
}
pub fn projected(projection: &Projection, row: usize) -> Option<usize> {
    match projection.row(row) {
        Some(DisplayRow::Text(r)) => Some(r),
        _ => None,
    }
}
