//! Typed, GUI-independent table comparison. All worker inputs are immutable snapshots.
#![forbid(unsafe_code)]
pub mod model;
pub mod regional;
pub mod source;
pub mod workbook;
pub use bc_formats::TableOptions;
pub use model::{Cell, CellValue, MergedRange, Sheet, Style, StyleId, TableModel};

pub mod columns;
pub mod inference;
pub mod rules;
pub mod sheets;
pub use rules::{ColumnHandling, TableRules};

pub type Result<T> = std::result::Result<T, bc_core::CoreError>;
pub(crate) fn invalid(message: impl Into<String>) -> bc_core::CoreError {
    bc_core::CoreError::Invalid(message.into())
}
