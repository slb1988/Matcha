//! 文本合并视图（三方）：数据模型与界面。合并语义全部来自 `bc-engine-text::merge`。
pub mod model;
pub mod view;

pub use model::{MergeLaunch, MergeModel};
pub use view::{MergeAction, MergeView};
