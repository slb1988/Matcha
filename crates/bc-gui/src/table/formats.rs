//! Matcha-owned format registry and table options; never execute converters.
//! Which comparison a path opens in is decided by `bc_formats::route`.
use bc_core::CoreError;
use bc_engine_table::source::ReadOptions;
use bc_formats::registry::FormatRegistry;
use std::path::Path;

pub(crate) fn load_registry() -> Result<FormatRegistry, CoreError> {
    match bc_formats::registry::user_formats_dir() {
        Ok(path) => FormatRegistry::load_user_dir(&path)
            .map_err(|e| CoreError::Invalid(format!("user formats: {e}"))),
        Err(_) => Ok(FormatRegistry::with_builtins()),
    }
}
/// 表格读取选项：与脚本报告/合并共用 [`bc_script::table_report::read_options`]。
pub(crate) fn options(path: &Path, registry: &FormatRegistry) -> Result<ReadOptions, CoreError> {
    bc_script::table_report::read_options(path, registry).map_err(|e| CoreError::Invalid(e.message))
}
