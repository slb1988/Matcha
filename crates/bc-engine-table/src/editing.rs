//! Reversible structural edits are wrapped by TableMap::edit_sheet / EditStack.
use crate::{invalid, model::RawRow, Cell, Result, Sheet};
use std::{collections::HashSet, sync::Arc};
impl Sheet {
    pub fn remove_rows(&mut self, range: std::ops::Range<usize>) -> Result<()> {
        if range.start > range.end || range.end > self.row_count {
            return Err(invalid("row range out of bounds"));
        }
        let order = (0..self.row_count)
            .filter(|r| !range.contains(r))
            .collect::<Vec<_>>();
        self.retain_rows(&order)?;
        if self.row_count > 0 {
            self.row_edit_marks
                .insert(range.start.min(self.row_count - 1));
        }
        Ok(())
    }
    pub fn reorder_rows(&mut self, order: &[usize]) -> Result<()> {
        if order.len() != self.row_count {
            return Err(invalid("row permutation length mismatch"));
        }
        self.retain_rows(order)?;
        self.row_edit_marks.extend(0..self.row_count);
        Ok(())
    }
    fn retain_rows(&mut self, order: &[usize]) -> Result<()> {
        let seen: HashSet<_> = order.iter().copied().collect();
        if seen.len() != order.len() || order.iter().any(|&r| r >= self.row_count) {
            return Err(invalid("invalid row permutation"));
        }
        let mut inverse = vec![None; self.row_count];
        for (r, &old) in order.iter().enumerate() {
            inverse[old] = Some(r);
        }
        for col in &mut self.columns {
            *col = Arc::new(order.iter().map(|&r| col[r].clone()).collect());
        }
        self.changed = self
            .changed
            .iter()
            .filter_map(|&(r, c)| inverse[r].map(|r| (r, c)))
            .collect();
        self.display_overrides = self
            .display_overrides
            .iter()
            .filter_map(|(&(r, c), v)| inverse[r].map(|r| ((r, c), v.clone())))
            .collect();
        self.row_edit_marks = self
            .row_edit_marks
            .iter()
            .filter_map(|&r| inverse[r])
            .collect();
        if let Some(source) = &mut self.text_source {
            if !source.rows.is_empty() {
                let mut rows = order
                    .iter()
                    .map(|&r| source.rows.get(r).cloned().unwrap_or_default())
                    .collect::<Vec<_>>();
                let count = rows.len();
                for row in rows.iter_mut().take(count.saturating_sub(1)) {
                    if row.ending.is_empty() {
                        row.ending = source.default_ending.clone();
                    }
                }
                source.rows = Arc::new(rows);
            }
        }
        self.row_count = order.len();
        self.merged_ranges.clear();
        self.fresh_revision();
        Ok(())
    }
    pub fn insert_column(&mut self, at: usize, name: String) -> Result<()> {
        if at > self.columns.len() {
            return Err(invalid("column insertion out of bounds"));
        }
        let mut source = self
            .source_column_map
            .take()
            .unwrap_or_else(|| (0..self.columns.len()).map(Some).collect());
        source.insert(at, None);
        self.source_column_map = Some(source);
        self.columns
            .insert(at, Arc::new(vec![Cell::default(); self.row_count]));
        if let Some(headers) = &mut self.headers {
            headers.resize(self.columns.len() - 1, String::new());
            headers.insert(at, name);
        }
        self.shift_columns(at, true);
        self.row_edit_marks.extend(0..self.row_count);
        self.merged_ranges.clear();
        self.fresh_revision();
        Ok(())
    }
    pub fn remove_column(&mut self, at: usize) -> Result<()> {
        if at >= self.columns.len() {
            return Err(invalid("column deletion out of bounds"));
        }
        let mut source = self
            .source_column_map
            .take()
            .unwrap_or_else(|| (0..self.columns.len()).map(Some).collect());
        source.remove(at);
        self.source_column_map = Some(source);
        self.columns.remove(at);
        if let Some(headers) = &mut self.headers {
            if at < headers.len() {
                headers.remove(at);
            }
        }
        self.shift_columns(at, false);
        self.row_edit_marks.extend(0..self.row_count);
        self.merged_ranges.clear();
        self.fresh_revision();
        Ok(())
    }
    fn shift_columns(&mut self, at: usize, insert: bool) {
        let shift = |c| {
            if insert {
                Some(if c >= at { c + 1 } else { c })
            } else if c == at {
                None
            } else {
                Some(if c > at { c - 1 } else { c })
            }
        };
        self.changed = self
            .changed
            .iter()
            .filter_map(|&(r, c)| shift(c).map(|c| (r, c)))
            .collect();
        self.display_overrides = self
            .display_overrides
            .iter()
            .filter_map(|(&(r, c), v)| shift(c).map(|c| ((r, c), v.clone())))
            .collect();
    }
    /// One atomic rectangular paste; callers supply decoded cell values.
    pub fn paste_cells(&mut self, row: usize, col: usize, values: Vec<Vec<Cell>>) -> Result<()> {
        let width = values.iter().map(Vec::len).max().unwrap_or(0);
        while self.column_count() < col + width {
            self.insert_column(self.column_count(), String::new())?;
        }
        while self.row_count() < row + values.len() {
            self.insert_row(self.row_count(), Vec::new())?;
        }
        for (r, values) in values.into_iter().enumerate() {
            for (c, value) in values.into_iter().enumerate() {
                self.set_cell(row + r, col + c, value)?;
            }
        }
        Ok(())
    }
    pub fn source_is_workbook(&self) -> bool {
        matches!(self.source, crate::model::SheetSource::Workbook)
    }
    pub fn estimated_metadata_bytes(&self) -> usize {
        self.row_count * self.column_count() * std::mem::size_of::<Cell>()
            + self
                .text_source
                .as_ref()
                .map_or(0, |s| s.rows.len() * std::mem::size_of::<RawRow>())
    }
}
