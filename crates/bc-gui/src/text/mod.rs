//! 正式的虚拟化文本比较控件；所有比较语义来自 bc-engine-text。
pub mod editor;
pub mod find_panel;
mod format_panel;
pub mod gutter;
pub mod nav_bar;
pub mod projection;
mod render;
pub mod settings_panel;
pub mod status_bar;
pub mod view;
mod worker;

pub use view::{TextCompareView, ViewAction};
