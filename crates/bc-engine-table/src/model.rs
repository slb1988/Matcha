use crate::{invalid, Result};
use bc_core::encoding::Encoding;
use bc_formats::{model::RegionalSettings, TableOptions};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

static REVISION: AtomicU64 = AtomicU64::new(1);
pub(crate) fn revision() -> u64 {
    REVISION.fetch_add(1, Ordering::Relaxed)
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum CellValue {
    #[default]
    Empty,
    Text(Arc<str>),
    Number(f64),
    Bool(bool),
    /// Milliseconds since Unix epoch, without an implicit time zone.
    Date(i64),
    Error(Arc<str>),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StyleId(pub u32);
/// Only number formats are compared in this change; fonts/fills remain reserved.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Style {
    pub number_format: Option<Arc<str>>,
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Cell {
    pub value: CellValue,
    pub formula: Option<Arc<str>>,
    pub style: Option<StyleId>,
}
impl Cell {
    pub fn new(value: CellValue) -> Self {
        Self {
            value,
            ..Self::default()
        }
    }
    pub fn text(text: impl Into<Arc<str>>) -> Self {
        Self::new(CellValue::Text(text.into()))
    }
    pub fn number(value: f64) -> Self {
        Self::new(CellValue::Number(value))
    }
    pub fn display(&self) -> String {
        match &self.value {
            CellValue::Empty => String::new(),
            CellValue::Text(s) | CellValue::Error(s) => s.to_string(),
            CellValue::Number(n) => n.to_string(),
            CellValue::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
            CellValue::Date(ms) => crate::regional::format_date(*ms),
        }
    }
}
/// Inclusive absolute source coordinates (zero based).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MergedRange {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RawRow {
    pub text: Arc<str>,
    pub fields: Vec<(u32, u32)>,
    pub ending: Arc<str>,
}
#[derive(Debug, Clone)]
pub(crate) struct TextSource {
    pub options: TableOptions,
    pub encoding: Encoding,
    pub has_bom: bool,
    pub header: Option<RawRow>,
    pub rows: Arc<Vec<RawRow>>,
    pub default_ending: Arc<str>,
}
#[derive(Debug, Clone)]
pub enum SheetSource {
    Memory,
    Text,
    Workbook,
}

/// Columnar snapshot. Cloning shares columns; validated edits copy only affected
/// columns and issue globally unique revisions (also on undo/redo).
#[derive(Debug, Clone)]
pub struct Sheet {
    pub name: String,
    pub headers: Option<Vec<String>>,
    pub merged_ranges: Vec<MergedRange>,
    pub regional: RegionalSettings,
    pub source: SheetSource,
    pub(crate) columns: Vec<Arc<Vec<Cell>>>,
    pub(crate) row_count: usize,
    pub(crate) styles: Arc<Vec<Style>>,
    pub(crate) text_source: Option<TextSource>,
    pub(crate) changed: HashSet<(usize, usize)>,
    pub(crate) display_overrides: HashMap<(usize, usize), Arc<str>>,
    pub(crate) source_column_map: Option<Vec<Option<usize>>>,
    pub(crate) row_edit_marks: HashSet<usize>,
    pub(crate) revision: u64,
}
impl Default for Sheet {
    fn default() -> Self {
        Self::new("Sheet1")
    }
}
impl Sheet {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            headers: None,
            merged_ranges: Vec::new(),
            regional: RegionalSettings::default(),
            source: SheetSource::Memory,
            columns: Vec::new(),
            row_count: 0,
            styles: Arc::new(Vec::new()),
            text_source: None,
            changed: HashSet::new(),
            display_overrides: HashMap::new(),
            source_column_map: None,
            row_edit_marks: HashSet::new(),
            revision: revision(),
        }
    }
    pub fn row_count(&self) -> usize {
        self.row_count
    }
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn cell(&self, row: usize, column: usize) -> Option<&Cell> {
        self.columns.get(column)?.get(row)
    }
    pub fn column(&self, column: usize) -> Option<&[Cell]> {
        self.columns.get(column).map(|c| c.as_slice())
    }
    pub fn column_name(&self, column: usize) -> String {
        self.headers
            .as_ref()
            .and_then(|h| h.get(column))
            .cloned()
            .unwrap_or_else(|| (column + 1).to_string())
    }
    pub fn style(&self, id: Option<StyleId>) -> Option<&Style> {
        self.styles.get(id?.0 as usize)
    }
    pub fn intern_style(&mut self, style: Style) -> StyleId {
        if let Some(i) = self.styles.iter().position(|s| s == &style) {
            return StyleId(i as u32);
        }
        let i = self.styles.len();
        Arc::make_mut(&mut self.styles).push(style);
        self.revision = revision();
        StyleId(i as u32)
    }
    pub fn push_row(&mut self, cells: Vec<Cell>) {
        while self.columns.len() < cells.len() {
            self.columns
                .push(Arc::new(vec![Cell::default(); self.row_count]));
        }
        for (i, column) in self.columns.iter_mut().enumerate() {
            Arc::make_mut(column).push(cells.get(i).cloned().unwrap_or_default());
        }
        self.row_count += 1;
        self.revision = revision();
    }
    pub fn set_cell(&mut self, row: usize, column: usize, cell: Cell) -> Result<()> {
        if row >= self.row_count || column >= self.columns.len() {
            return Err(invalid("cell out of range"));
        }
        if let Some(id) = cell.style {
            if self.style(Some(id)).is_none() {
                return Err(invalid("unknown style id"));
            }
        }
        Arc::make_mut(&mut self.columns[column])[row] = cell;
        self.changed.insert((row, column));
        self.display_overrides.remove(&(row, column));
        self.revision = revision();
        Ok(())
    }
    /// Insert a source row, preserving unrelated lexical records and edit marks.
    pub fn insert_row(&mut self, at: usize, cells: Vec<Cell>) -> Result<()> {
        if at > self.row_count {
            return Err(invalid("row insertion out of range"));
        }
        if cells
            .iter()
            .any(|c| c.style.is_some() && self.style(c.style).is_none())
        {
            return Err(invalid("unknown style id"));
        }
        self.ensure_size(self.row_count, cells.len());
        for (column, values) in self.columns.iter_mut().enumerate() {
            Arc::make_mut(values).insert(at, cells.get(column).cloned().unwrap_or_default());
        }
        self.changed = self
            .changed
            .iter()
            .map(|&(row, col)| (if row >= at { row + 1 } else { row }, col))
            .collect();
        self.display_overrides = self
            .display_overrides
            .iter()
            .map(|(&(row, col), text)| ((if row >= at { row + 1 } else { row }, col), text.clone()))
            .collect();
        self.row_edit_marks = self
            .row_edit_marks
            .iter()
            .map(|&row| if row >= at { row + 1 } else { row })
            .collect();
        for column in 0..self.columns.len() {
            self.changed.insert((at, column));
        }
        if let Some(source) = &mut self.text_source {
            if !source.rows.is_empty() {
                let rows = Arc::make_mut(&mut source.rows);
                if at == rows.len() && at > 0 && rows[at - 1].ending.is_empty() {
                    rows[at - 1].ending = source.default_ending.clone();
                }
                rows.insert(at, RawRow::default());
            }
        }
        self.row_count += 1;
        self.revision = revision();
        Ok(())
    }
    pub fn is_edited(&self, row: usize) -> bool {
        self.row_edit_marks.contains(&row) || self.changed.iter().any(|&(r, _)| r == row)
    }
    /// Source-loading header promotion, before creating an edit history.
    pub fn take_header(&mut self) {
        if self.row_count == 0 {
            return;
        }
        self.headers = Some(
            self.columns
                .iter_mut()
                .map(|c| Arc::make_mut(c).remove(0).display())
                .collect(),
        );
        self.row_count -= 1;
        self.revision = revision();
    }
    /// Export text suitable for summaries: preserve the original field spelling
    /// for untouched cells, including numeric trailing zeroes.
    pub fn display_at(&self, row: usize, column: usize) -> Option<String> {
        let cell = self.cell(row, column)?;
        if let Some(text) = self.display_overrides.get(&(row, column)) {
            return Some(text.to_string());
        }
        if !self.changed.contains(&(row, column)) {
            if let Some(source) = &self.text_source {
                if let Some(raw) = source.rows.get(row) {
                    let original = self
                        .source_column_map
                        .as_ref()
                        .and_then(|m| m.get(column))
                        .copied()
                        .unwrap_or(Some(column));
                    if let Some(&(start, end)) = original.and_then(|c| raw.fields.get(c)) {
                        return Some(crate::source::field_text(
                            &raw.text[start as usize..end as usize],
                            &source.options,
                        ));
                    }
                }
            }
        }
        Some(cell.display())
    }
    /// Bounded rendering preview without cloning an entire large textual cell.
    pub fn preview(&self, row: usize, column: usize, start: usize, limit: usize) -> Option<String> {
        if let Some(text) = self.display_overrides.get(&(row, column)) {
            return Some(text.chars().skip(start).take(limit).collect());
        }
        match &self.cell(row, column)?.value {
            CellValue::Text(text) | CellValue::Error(text) => {
                Some(text.chars().skip(start).take(limit).collect())
            }
            _ => self
                .display_at(row, column)
                .map(|text| text.chars().skip(start).take(limit).collect()),
        }
    }
    pub(crate) fn ensure_size(&mut self, rows: usize, columns: usize) {
        while self.columns.len() < columns {
            self.columns
                .push(Arc::new(vec![Cell::default(); self.row_count]));
        }
        if rows > self.row_count {
            for c in &mut self.columns {
                Arc::make_mut(c).resize(rows, Cell::default());
            }
            self.row_count = rows;
        }
    }
    pub(crate) fn put_loaded(&mut self, row: usize, column: usize, cell: Cell) {
        self.ensure_size(row + 1, column + 1);
        Arc::make_mut(&mut self.columns[column])[row] = cell;
    }
    pub(crate) fn fresh_revision(&mut self) {
        self.revision = revision();
    }
}

/// Workbook/table container. Lazy workbook handles live in `workbook::Workbook`,
/// not in snapshots sent to comparison threads.
#[derive(Debug, Clone, Default)]
pub struct TableModel {
    pub sheets: Vec<Sheet>,
}
impl TableModel {
    pub fn new(sheet: Sheet) -> Self {
        Self {
            sheets: vec![sheet],
        }
    }
}

/// Bounded interning: repeating textual values share storage, but high-cardinality
/// columns do not retain a second unbounded index beside the model.
#[derive(Default)]
pub(crate) struct StringPool(HashMap<String, Arc<str>>);
impl StringPool {
    pub fn intern(&mut self, value: &str) -> Arc<str> {
        if let Some(existing) = self.0.get(value) {
            return existing.clone();
        }
        let shared: Arc<str> = value.into();
        if self.0.len() < 65_536 {
            self.0.insert(value.into(), shared.clone());
        }
        shared
    }
}
