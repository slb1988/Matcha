//! Matcha-owned format masks and table options; never execute converters.
use bc_core::CoreError;
use bc_engine_table::source::ReadOptions;
use bc_formats::{model::ConvertKind, registry::FormatRegistry, FormatKind};
use std::path::Path;

pub(crate) fn load_registry() -> Result<FormatRegistry, CoreError> {
    match bc_formats::registry::user_formats_dir() {
        Ok(path) => FormatRegistry::load_user_dir(&path)
            .map_err(|e| CoreError::Invalid(format!("user formats: {e}"))),
        Err(_) => Ok(FormatRegistry::with_builtins()),
    }
}
fn extension(path: &Path, names: &[&str]) -> bool {
    path.extension()
        .is_some_and(|e| names.iter().any(|name| e.eq_ignore_ascii_case(name)))
}
pub(crate) fn table_path(path: &Path, registry: &FormatRegistry) -> bool {
    if extension(path, &["xlsx", "xlsm"]) {
        return true; // Native workbook routing never invokes an external converter.
    }
    registry.resolve(path, None).map_or_else(
        || extension(path, &["csv", "tsv"]),
        |format| format.kind == FormatKind::Table,
    )
}
pub(crate) fn options(path: &Path, registry: &FormatRegistry) -> Result<ReadOptions, CoreError> {
    if extension(path, &["xls", "ods"]) {
        return Err(CoreError::Invalid(
            "This workbook type is not supported; use XLSX/XLSM or CSV".into(),
        ));
    }
    let mut options = ReadOptions::default();
    if let Some(format) = registry.resolve(path, Some(FormatKind::Table)) {
        if format
            .convert
            .is_some_and(|c| c != ConvertKind::MsExcel || !extension(path, &["xlsx", "xlsm"]))
        {
            return Err(CoreError::Invalid(
                "Table external conversion is not supported".into(),
            ));
        }
        options.format = format.table.clone().unwrap_or_default();
    } else if extension(path, &["tsv"]) {
        options.format.delimiters = vec!['\t'];
    } else {
        options.format.detect_delimiter = true;
    }
    Ok(options)
}
