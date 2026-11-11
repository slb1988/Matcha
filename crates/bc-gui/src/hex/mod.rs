//! Hex Compare 视图（hex-compare change）。
//!
//! 网格渲染/交互与会话设置分布在 `render`/`view` 等模块；后台对齐见 [`worker`]。

pub mod render;
pub mod view;
pub mod worker;

pub use view::HexCompareView;

/// 默认文件锁阈值（2 MB，对齐参考工具）。
pub fn default_lock_threshold() -> u64 {
    bc_engine_bin::hex::HexRules::default().lock_threshold
}
