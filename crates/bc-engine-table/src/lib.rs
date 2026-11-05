//! # bc-engine-table
//!
//! Table Compare engine (FR-C1, M2): delimited text (CSV) first, Excel via converter later.
//!
//! | type | BC classes |
//! |---|---|
//! | [`TableModel`] / [`TableRow`] | `TTableContent` (`TLineContent`), `TTableDefinition` |
//! | [`TableRules`] / [`ColumnHandling`] | `TTableRules` (`TLineRules`), `TTableSessionColumnsFrame`, `TTableSessionRowsFrame`, `TTableSessionSheetsFrame`, `TTableColHandlingDialog` |
//! | [`KeyAlign`] / [`TableMap`] | `TTableMap`, `TTableMapAlignLines` (`TBcAlignLines`), `TTableMapAlignJob`, `TTableMapRealignJob`, `TTableMapSortJob`, `TTableMapSortList` |
//! | [`jobs`] | `TTableMapJob`, `TTableColDetectSizeJob` family (`TTableColWidthsDetector`) |
#![allow(unused_variables)] // scaffold: bodies are `todo!()`; remove once implemented
#![forbid(unsafe_code)]

use bc_core::align::AlignMap;
use bc_formats::TableOptions;

/// One cell.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Cell {
    pub text: String,
}

/// One row of cells.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TableRow {
    pub cells: Vec<Cell>,
    /// Source line number (1-based) for reports (`line-numbers` option).
    pub source_line: usize,
}

/// Parsed table of one side.
#[derive(Debug, Clone, Default)]
pub struct TableModel {
    pub headers: Option<Vec<String>>,
    pub rows: Vec<TableRow>,
    pub column_count: usize,
}

impl TableModel {
    /// Parse delimited text using the format's delimiter/qualifier settings.
    pub fn parse_delimited(text: &str, options: &TableOptions, first_row_is_header: bool) -> Self {
        todo!("bc-engine-table: delimited parser (quoting, embedded newlines)")
    }
}

/// Per-column handling – `TTableColHandlingDialog`.
#[derive(Debug, Clone, PartialEq)]
pub struct ColumnHandling {
    pub index: usize,
    /// Part of the row key used for alignment.
    pub is_key: bool,
    /// Compare as number (tolerance) instead of text.
    pub numeric: bool,
    pub numeric_tolerance: f64,
    /// Differences in this column are unimportant.
    pub unimportant: bool,
    /// Column ignored entirely.
    pub ignore: bool,
    pub case_sensitive: bool,
}

/// Table session rules – `TTableRules`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableRules {
    pub first_row_is_header: bool,
    pub columns: Vec<ColumnHandling>,
    /// Sort rows by key before aligning (`TTableMapSortJob`).
    pub sort_by_key: bool,
    /// Sheet name selection for multi-sheet sources (`TTableSessionSheetsFrame`).
    pub sheet: Option<String>,
    /// Cells considered unimportant when they differ only by these regex replacements.
    pub replacements: Vec<bc_formats::text_rules::Replacement>,
}

/// Aligns rows by key columns – backend of `TTableMapAlignLines`.
#[derive(Debug, Default)]
pub struct KeyAlign {
    pub key_columns: Vec<usize>,
    pub sorted: bool,
}

impl KeyAlign {
    /// Align two tables; rows with equal keys are paired, others become orphans.
    pub fn align(&self, left: &TableModel, right: &TableModel) -> AlignMap {
        todo!("bc-engine-table: key alignment (hash keys, fall back to positional)")
    }
}

/// Aligned pair of tables – `TTableMap`.
#[derive(Debug, Default)]
pub struct TableMap {
    pub left: TableModel,
    pub right: TableModel,
    pub rules: TableRules,
    pub align: AlignMap,
}

impl TableMap {
    /// Per-cell difference classification for one aligned row.
    pub fn cell_diffs(&self, row: usize) -> Vec<bc_core::DiffKind> {
        todo!("bc-engine-table: cell comparison with column handling")
    }
}

/// Background jobs for the table engine.
pub mod jobs {
    use super::{TableModel, TableRules};
    use bc_jobs::{Job, JobContext, JobResult};

    /// `TTableMapAlignJob` – parse + align.
    pub struct TableAlignJob {
        pub left: TableModel,
        pub right: TableModel,
        pub rules: TableRules,
    }

    impl Job for TableAlignJob {
        fn name(&self) -> String {
            "Aligning table".into()
        }

        fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
            todo!("bc-engine-table::jobs: sort + key align")
        }
    }

    /// `TTableColDetectSizeJob` – detect display column widths.
    pub struct DetectColumnWidthsJob {
        pub model: TableModel,
        pub sample_rows: usize,
    }

    impl Job for DetectColumnWidthsJob {
        fn name(&self) -> String {
            "Detecting column widths".into()
        }

        fn run(self: Box<Self>, ctx: &JobContext) -> JobResult {
            todo!("bc-engine-table::jobs: width detection")
        }
    }
}
