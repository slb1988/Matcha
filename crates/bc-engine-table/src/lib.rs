//! Typed, GUI-independent table comparison. All worker inputs are immutable snapshots.
#![forbid(unsafe_code)]
pub mod model;
mod perf;
pub mod regional;
pub mod source;
pub mod workbook;
pub use bc_formats::TableOptions;
pub use model::{Cell, CellValue, MergedRange, Sheet, Style, StyleId, TableModel};

pub mod align;
pub mod compare;
mod copying;
pub mod editing;
pub mod jobs;
pub mod map;
pub mod merge;
pub mod xlsx_write;
pub use align::KeyAlign;
pub use data::{CellData, CellInfo, TableData};
pub use map::TableMap;
pub mod columns;
pub mod data;
pub mod demand;
pub mod inference;
pub mod rules;
pub mod sheets;
pub mod store;
pub mod summary;
pub use rules::{ColumnHandling, TableRules};
pub use store::SheetStore;

pub type Result<T> = std::result::Result<T, bc_core::CoreError>;
pub(crate) fn invalid(message: impl Into<String>) -> bc_core::CoreError {
    bc_core::CoreError::Invalid(message.into())
}
