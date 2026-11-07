//! Table storage bridge: one enum over the default dense `Sheet` and the
//! experimental on-demand `DemandSheet`. This is a table-storage bridge only —
//! not a general database/configuration layer. Dense behavior is unchanged
//! (delegation to the same `Sheet` methods); the GUI model (`TableMap`) holds
//! `SheetStore` so an explicit experimental entry can bind a CSV side to
//! demand storage while every other path keeps dense sheets.

use std::{collections::HashSet, sync::Arc};

use bc_formats::model::RegionalSettings;

use crate::{
    data::{CellInfo, TableData},
    demand::DemandSheet,
    Cell, ColumnHandling, Result, Sheet, Style, StyleId,
};

/// One comparison side's storage. Clone is cheap for both variants (arc-shared
/// columns / arc-shared mapping + version), matching prior `Sheet` clone cost
/// semantics relied on by edit snapshots and worker hand-offs.
#[derive(Debug, Clone)]
pub enum SheetStore {
    Dense(Sheet),
    Lazy(DemandSheet),
}
impl Default for SheetStore {
    fn default() -> Self {
        Self::Dense(Sheet::default())
    }
}

impl SheetStore {
    /// Sheet/tab name (dense `Sheet::name` parity).
    pub fn name(&self) -> String {
        match self {
            Self::Dense(s) => s.name.clone(),
            Self::Lazy(s) => s.name().to_string(),
        }
    }
    pub fn row_count(&self) -> usize {
        match self {
            Self::Dense(s) => s.row_count(),
            Self::Lazy(s) => s.row_count(),
        }
    }
    pub fn column_count(&self) -> usize {
        match self {
            Self::Dense(s) => s.column_count(),
            Self::Lazy(s) => s.column_count(),
        }
    }
    pub fn column_name(&self, column: usize) -> String {
        match self {
            Self::Dense(s) => s.column_name(column),
            Self::Lazy(s) => s.column_name(column),
        }
    }
    pub fn display_at(&self, row: usize, column: usize) -> Option<String> {
        match self {
            Self::Dense(s) => s.display_at(row, column),
            Self::Lazy(s) => s.display_text_at(row, column),
        }
    }
    pub fn preview(&self, row: usize, column: usize, start: usize, limit: usize) -> Option<String> {
        match self {
            Self::Dense(s) => s.preview(row, column, start, limit),
            Self::Lazy(s) => s.preview(row, column, start, limit),
        }
    }
    /// Owned cell snapshot (`Sheet::cell(...).cloned()` equivalent); the demand
    /// variant cannot hand out references into parsed-on-access storage.
    pub fn cell_snapshot(&self, row: usize, column: usize) -> Option<Cell> {
        match self {
            Self::Dense(s) => s.cell(row, column).cloned(),
            Self::Lazy(s) => s.cell_snapshot(row, column),
        }
    }
    pub fn style(&self, id: Option<StyleId>) -> Option<&Style> {
        match self {
            Self::Dense(s) => s.style(id),
            Self::Lazy(s) => s.style(id),
        }
    }
    pub fn intern_style(&mut self, style: Style) -> StyleId {
        match self {
            Self::Dense(s) => s.intern_style(style),
            Self::Lazy(s) => s.intern_style(style),
        }
    }
    pub fn revision(&self) -> u64 {
        match self {
            Self::Dense(s) => s.revision(),
            Self::Lazy(s) => s.revision(),
        }
    }
    pub fn fresh_revision(&mut self) {
        match self {
            Self::Dense(s) => s.fresh_revision(),
            Self::Lazy(s) => s.fresh_revision(),
        }
    }
    pub fn is_edited(&self, row: usize) -> bool {
        match self {
            Self::Dense(s) => s.is_edited(row),
            Self::Lazy(s) => s.is_edited(row),
        }
    }
    pub fn source_is_workbook(&self) -> bool {
        match self {
            Self::Dense(s) => s.source_is_workbook(),
            Self::Lazy(s) => s.source_is_workbook(),
        }
    }
    pub fn set_cell(&mut self, row: usize, column: usize, cell: Cell) -> Result<()> {
        match self {
            Self::Dense(s) => s.set_cell(row, column, cell),
            Self::Lazy(s) => s.set_cell(row, column, cell),
        }
    }
    pub fn insert_row(&mut self, at: usize, cells: Vec<Cell>) -> Result<()> {
        match self {
            Self::Dense(s) => s.insert_row(at, cells),
            Self::Lazy(s) => s.insert_row(at, cells),
        }
    }
    pub fn remove_row_indices(&mut self, rows: &[usize]) -> Result<()> {
        match self {
            Self::Dense(s) => s.remove_row_indices(rows),
            Self::Lazy(s) => s.remove_row_indices(rows),
        }
    }
    pub fn reorder_rows(&mut self, order: &[usize]) -> Result<()> {
        match self {
            Self::Dense(s) => s.reorder_rows(order),
            Self::Lazy(s) => s.reorder_rows(order),
        }
    }
    pub fn sorted_row_order(
        &self,
        rows: &[usize],
        keys: &[(usize, ColumnHandling)],
        descending: bool,
    ) -> Result<Option<Vec<usize>>> {
        match self {
            Self::Dense(s) => s.sorted_row_order(rows, keys, descending),
            Self::Lazy(s) => s.sorted_row_order(rows, keys, descending),
        }
    }
    pub fn insert_column(&mut self, at: usize, name: String) -> Result<()> {
        match self {
            Self::Dense(s) => s.insert_column(at, name),
            Self::Lazy(s) => s.insert_column(at, name),
        }
    }
    pub fn remove_column(&mut self, at: usize) -> Result<()> {
        match self {
            Self::Dense(s) => s.remove_column(at),
            Self::Lazy(s) => s.remove_column(at),
        }
    }
    pub fn paste_cells(&mut self, row: usize, col: usize, values: Vec<Vec<Cell>>) -> Result<()> {
        match self {
            Self::Dense(s) => s.paste_cells(row, col, values),
            Self::Lazy(s) => s.paste_cells(row, col, values),
        }
    }
    /// Dense `ensure_size` parity (copy_rows column growth): silent, no marks.
    pub(crate) fn ensure_size(&mut self, rows: usize, columns: usize) {
        match self {
            Self::Dense(s) => s.ensure_size(rows, columns),
            Self::Lazy(s) => s.ensure_size(rows, columns),
        }
    }
    /// `copying.rs` parity: copy operations record the source display text as
    /// an override on the target cell.
    pub(crate) fn insert_display_override(&mut self, row: usize, column: usize, text: Arc<str>) {
        match self {
            Self::Dense(s) => {
                s.display_overrides.insert((row, column), text);
            }
            Self::Lazy(s) => s.insert_display_override(row, column, text),
        }
    }
    /// Worker-snapshot compare supply mode; no-op for dense storage.
    pub(crate) fn set_compare_raw_supply(&self, raw: bool) {
        if let Self::Lazy(s) = self {
            s.set_compare_raw_supply(raw);
        }
    }
    /// Mapped virtual bytes of the demand snapshot (0 for dense). Virtual size,
    /// never residency.
    pub fn mapped_virtual_bytes(&self) -> u64 {
        match self {
            Self::Dense(_) => 0,
            Self::Lazy(s) => s.mapped_virtual_bytes(),
        }
    }
}

impl TableData for SheetStore {
    fn row_count(&self) -> usize {
        SheetStore::row_count(self)
    }
    fn column_count(&self) -> usize {
        SheetStore::column_count(self)
    }
    fn cell_info(&self, row: usize, column: usize) -> Option<CellInfo<'_>> {
        match self {
            Self::Dense(s) => TableData::cell_info(s, row, column),
            Self::Lazy(s) => TableData::cell_info(s, row, column),
        }
    }
    fn display_into(&self, row: usize, column: usize, out: &mut String) {
        match self {
            Self::Dense(s) => TableData::display_into(s, row, column, out),
            Self::Lazy(s) => TableData::display_into(s, row, column, out),
        }
    }
    fn display_text(&self, row: usize, column: usize) -> String {
        match self {
            Self::Dense(s) => TableData::display_text(s, row, column),
            Self::Lazy(s) => TableData::display_text(s, row, column),
        }
    }
    fn headers(&self) -> Option<&[String]> {
        match self {
            Self::Dense(s) => TableData::headers(s),
            Self::Lazy(s) => TableData::headers(s),
        }
    }
    fn regional(&self) -> &RegionalSettings {
        match self {
            Self::Dense(s) => TableData::regional(s),
            Self::Lazy(s) => TableData::regional(s),
        }
    }
    fn revision(&self) -> u64 {
        SheetStore::revision(self)
    }
    fn collect_edited_rows(&self, out: &mut HashSet<usize>) {
        match self {
            Self::Dense(s) => TableData::collect_edited_rows(s, out),
            Self::Lazy(s) => TableData::collect_edited_rows(s, out),
        }
    }
}

/// Save through the store: dense keeps `source::save_as`, demand streams from
/// its snapshot with the same staged atomic replacement protocol.
pub fn save_store(store: &SheetStore, path: &std::path::Path) -> Result<()> {
    match store {
        SheetStore::Dense(s) => crate::source::save_as(s, path),
        SheetStore::Lazy(s) => s.save_as(path),
    }
}

/// Source-preserving write through the store (`write_delimited` semantics).
pub fn write_store_delimited(store: &SheetStore, writer: impl std::io::Write) -> Result<()> {
    match store {
        SheetStore::Dense(s) => crate::source::write_delimited(s, writer),
        SheetStore::Lazy(s) => s.write_delimited(writer),
    }
}

/// Export through the store: full re-serialization of the current view, no
/// source-byte preservation (dense `export_csv` semantics).
pub fn export_store(store: &SheetStore, writer: impl std::io::Write) -> Result<()> {
    match store {
        SheetStore::Dense(s) => crate::source::export_csv(s, writer),
        SheetStore::Lazy(s) => s.export_delimited(writer),
    }
}
