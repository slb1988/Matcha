//! 正式的虚拟化文本比较控件；所有比较语义来自 bc-engine-text。
pub mod bookmarks;
pub mod editor;
pub mod find_panel;
mod format_panel;
pub mod gutter;
mod menu;
pub mod nav_bar;
pub mod projection;
pub(crate) mod render;
pub mod session_dialog;
pub mod status_bar;
pub mod syntax_style;
pub mod view;
mod worker;

pub use view::{TextCompareView, ViewAction};
