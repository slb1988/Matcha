//! Read-only cell access abstraction over table storage.
//!
//! `Sheet` implements `TableData` by borrowing its materialized cells; the
//! demand-driven prototype storage implements it by parsing fields on access.
//! The infer/align/compare chain is generic over this trait; behavior on the
//! `Sheet` path is unchanged (same values, same display text, same bytes into
//! hashes — the differential digest in examples/demand_proto.rs asserts this).

use std::borrow::Cow;
use std::collections::HashSet;

use crate::{CellValue, Sheet};
use bc_formats::model::RegionalSettings;

/// Borrowing view of a cell value; mirrors `CellValue` variants without
/// requiring a materialized `Arc<str>` payload on the demand path.
#[derive(Debug, Clone, PartialEq)]
pub enum CellData<'a> {
    Empty,
    Text(Cow<'a, str>),
    Number(f64),
    Bool(bool),
    Date(i64),
    Error(Cow<'a, str>),
}

impl<'a> From<&'a CellValue> for CellData<'a> {
    fn from(value: &'a CellValue) -> Self {
        match value {
            CellValue::Empty => CellData::Empty,
            CellValue::Text(t) => CellData::Text(Cow::Borrowed(t.as_ref())),
            CellValue::Number(n) => CellData::Number(*n),
            CellValue::Bool(b) => CellData::Bool(*b),
            CellValue::Date(d) => CellData::Date(*d),
            CellValue::Error(e) => CellData::Error(Cow::Borrowed(e.as_ref())),
        }
    }
}

impl CellData<'_> {
    /// Owned-conversion entry for freshly interpreted values.
    pub fn from_owned_value(value: CellValue) -> CellData<'static> {
        match value {
            CellValue::Empty => CellData::Empty,
            CellValue::Text(t) => CellData::Text(Cow::Owned(t.to_string())),
            CellValue::Number(n) => CellData::Number(n),
            CellValue::Bool(b) => CellData::Bool(b),
            CellValue::Date(d) => CellData::Date(d),
            CellValue::Error(e) => CellData::Error(Cow::Owned(e.to_string())),
        }
    }
    /// Byte-identical to `Cell::display_into` for the equivalent value; both
    /// arms mirror that implementation (keep in sync).
    pub fn display_into(&self, out: &mut String) {
        match self {
            CellData::Empty => {}
            CellData::Text(s) | CellData::Error(s) => out.push_str(s),
            CellData::Number(n) => {
                use std::fmt::Write;
                let _ = write!(out, "{n}");
            }
            CellData::Bool(b) => out.push_str(if *b { "TRUE" } else { "FALSE" }),
            CellData::Date(ms) => out.push_str(&crate::regional::format_date(*ms)),
        }
    }
}

/// One cell as seen by the compare chain: value plus the two extra layers the
/// comparator scores (formula text, resolved number format).
#[derive(Debug, Clone)]
pub struct CellInfo<'a> {
    pub value: CellData<'a>,
    pub formula: Option<&'a str>,
    pub number_format: Option<&'a str>,
}

/// Read-only tabular access for the infer/align/compare chain.
pub trait TableData {
    fn row_count(&self) -> usize;
    fn column_count(&self) -> usize;
    fn cell_info(&self, row: usize, column: usize) -> Option<CellInfo<'_>>;
    /// Typed display (`Cell::display_into` semantics; no edit overrides).
    fn display_into(&self, row: usize, column: usize, out: &mut String);
    /// Override/source-aware display (`Sheet::display_at` semantics).
    fn display_text(&self, row: usize, column: usize) -> String;
    fn headers(&self) -> Option<&[String]>;
    fn regional(&self) -> &RegionalSettings;
    fn revision(&self) -> u64;
    /// Rows touched by edits (Sheet: `changed` + `row_edit_marks`).
    fn collect_edited_rows(&self, out: &mut HashSet<usize>);
}

impl TableData for Sheet {
    fn row_count(&self) -> usize {
        Sheet::row_count(self)
    }
    fn column_count(&self) -> usize {
        Sheet::column_count(self)
    }
    fn cell_info(&self, row: usize, column: usize) -> Option<CellInfo<'_>> {
        let cell = Sheet::cell(self, row, column)?;
        Some(CellInfo {
            value: CellData::from(&cell.value),
            formula: cell.formula.as_deref(),
            number_format: self
                .style(cell.style)
                .and_then(|s| s.number_format.as_deref()),
        })
    }
    fn display_into(&self, row: usize, column: usize, out: &mut String) {
        if let Some(cell) = Sheet::cell(self, row, column) {
            cell.display_into(out);
        }
    }
    fn display_text(&self, row: usize, column: usize) -> String {
        Sheet::display_at(self, row, column).unwrap_or_default()
    }
    fn headers(&self) -> Option<&[String]> {
        self.headers.as_deref()
    }
    fn regional(&self) -> &RegionalSettings {
        &self.regional
    }
    fn revision(&self) -> u64 {
        Sheet::revision(self)
    }
    fn collect_edited_rows(&self, out: &mut HashSet<usize>) {
        out.extend(self.changed.iter().map(|&(r, _)| r));
        out.extend(self.row_edit_marks.iter().copied());
    }
}
