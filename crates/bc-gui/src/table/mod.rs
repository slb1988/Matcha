//! 二维虚拟化表格视图；全部比较/编辑语义来自 bc-engine-table。
mod cache;
mod chrome_ui;
mod commands;
mod details;
mod find;
pub(crate) mod formats;
mod grid;
mod header;
mod menu;
mod regional;
mod selection;
mod settings_panel;
mod sheet_tabs;
mod sorting;
pub mod view;
mod worker;
pub use view::{TableAction, TableCompareView};
#[cfg(test)]
mod tests;
