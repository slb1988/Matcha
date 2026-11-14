//! 表格合并视图（三方）：数据模型与界面。合并语义全部来自 `bc-engine-table::merge`，
//! 读入与写出来自 `bc-script::table_merge`（与 `/automerge` 同一条路径）。
pub mod model;
pub mod view;

pub use model::TableMergeModel;
pub use view::TableMergeView;
