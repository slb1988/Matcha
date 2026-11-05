//! Typed, GUI-independent table comparison. All worker inputs are immutable snapshots.
#![forbid(unsafe_code)]
pub mod model;
pub mod regional;
pub mod source;
pub mod workbook;
pub use bc_formats::TableOptions;
pub use model::{Cell, CellValue, MergedRange, Sheet, Style, StyleId, TableModel};

// The comparison modules are added in the subsequent implementation groups.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableRules;

pub type Result<T> = std::result::Result<T, bc_core::CoreError>;
pub(crate) fn invalid(message: impl Into<String>) -> bc_core::CoreError {
    bc_core::CoreError::Invalid(message.into())
}
