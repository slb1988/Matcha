//! 二维虚拟化表格视图；全部比较/编辑语义来自 bc-engine-table。
mod cache;
mod commands;
mod details;
mod find;
mod grid;
mod header;
mod regional;
mod selection;
mod settings_panel;
mod sheet_tabs;
pub mod view;
mod worker;
pub use view::{TableAction, TableCompareView};
#[cfg(test)]
mod tests;
