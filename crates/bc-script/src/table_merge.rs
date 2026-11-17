//! 表格三方合并的宿主侧：按格式分派（文本合并 / 表格合并）、读入三方表格、写出合并结果，以及
//! `/automerge` 的无窗口执行。合并语义全部在 `bc_engine_table::merge`；GUI 的表格合并视图
//! 也用这里的读入与写出，两条路径的结果一致。
//!
//! 读取规则与表格比较一致（[`crate::table_report`]）：xlsx/xlsm 原生读取（不取表头，首行按
//! 数据合并并原样写回）；其余按文件格式库的表格格式读取。文本表格的表头由 center 决定一次，
//! 再按同一设置读 left/right，避免三方表头判定不一致使行号错位。
use crate::{cli::MergeSpec, runner::AutoMergeReport, table_report::read_options, ExitCode};
use bc_core::cancel::NeverCancel;
use bc_engine_table::{
    merge::{auto_merge, MergeInputs, TableMerge, TableMergeOptions},
    source::{read_path, write_delimited},
    workbook::{load_sheet_parallel, Workbook},
    xlsx_write, Sheet,
};
use bc_formats::model::HeaderMode;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

/// 合并调用按什么合并。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeKind {
    Text,
    Table,
}

fn extension_is(path: &Path, names: &[&str]) -> bool {
    path.extension()
        .is_some_and(|e| names.iter().any(|n| e.eq_ignore_ascii_case(n)))
}

/// xlsx/xlsm 工作簿（扩展名，或扩展名对不上但内容是 xlsx 包——版本控制工具的临时文件名
/// 常带 `#版本` 之类后缀）；判定统一在 [`bc_formats::route::is_workbook`]。
pub fn is_workbook(path: &Path) -> bool {
    bc_formats::route::is_workbook(path)
}

/// 合并调用的分派：三个输入都是工作簿 → 表格合并；都不是 → 文本合并，带 `/tablemerge` 时
/// 按文本表格做表格合并。混合格式、或 `/tablemerge` 遇到不支持的工作簿类型 → `Err`（100）。
pub fn merge_kind(spec: &MergeSpec, table_switch: bool) -> Result<MergeKind, String> {
    let inputs = [&spec.left, &spec.right, &spec.center];
    let books = inputs.map(|p| is_workbook(p));
    if books.iter().all(|b| *b) {
        return Ok(MergeKind::Table);
    }
    if books.iter().any(|b| *b) {
        return Err(
            "merge inputs have different formats: either all three are xlsx/xlsm workbooks \
             or none is"
                .into(),
        );
    }
    if !table_switch {
        return Ok(MergeKind::Text);
    }
    if let Some(path) = inputs.iter().find(|p| extension_is(p, &["xls", "ods"])) {
        return Err(format!(
            "{}: this workbook type is not supported; use XLSX/XLSM or CSV",
            path.display()
        ));
    }
    Ok(MergeKind::Table)
}

/// 读入的三方表格与写出所需的来源信息。
pub struct LoadedTableMerge {
    pub merge: TableMerge,
    /// 输入是工作簿（输出写 xlsx）；否则是文本表格（输出按 center 的格式写回）。
    pub workbook: bool,
    /// 输入中 xlsm 的宏工程（center 优先），输出为 xlsm 时原样带入。
    pub vba: Option<Vec<u8>>,
    /// 工作簿输入的原始字节（left、center、right）：以 center 为模板写出，保留样式与版式。
    pub packages: Option<[Vec<u8>; 3]>,
    pub warnings: Vec<String>,
}

fn check_file(path: &Path) -> Result<(), String> {
    if path.is_dir() {
        return Err(format!(
            "{} is a folder; a merge needs three files",
            path.display()
        ));
    }
    if !path.is_file() {
        return Err(format!("{} does not exist", path.display()));
    }
    Ok(())
}

fn load_workbook(path: &Path) -> Result<Vec<Sheet>, String> {
    let registry = crate::runner::default_formats();
    let options = read_options(path, &registry).map_err(|e| e.message)?;
    let cannot = |e: &dyn std::fmt::Display| format!("cannot read {}: {e}", path.display());
    let names = Workbook::open(path, Arc::new(NeverCancel))
        .map_err(|e| cannot(&e))?
        .sheet_names()
        .to_vec();
    let format = &options.format;
    names
        .iter()
        .map(|name| {
            let mut sheet = load_sheet_parallel(path, name, Arc::new(NeverCancel))
                .map_err(|e| cannot(&format!("sheet {name:?}: {e}")))?;
            sheet.regional = if format.regional.use_system {
                options.system_regional.clone()
            } else {
                format.regional.clone()
            };
            Ok(sheet)
        })
        .collect()
}

fn load_text(path: &Path, header: Option<bool>) -> Result<Sheet, String> {
    let registry = crate::runner::default_formats();
    let mut options = read_options(path, &registry).map_err(|e| e.message)?;
    if let Some(header) = header {
        options.header = Some(if header {
            HeaderMode::Names
        } else {
            HeaderMode::Data
        });
    }
    read_path(path, &options, &NeverCancel)
        .map(|output| output.sheet)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))
}

/// 读入三方表格并建立合并。输入整读进内存，所以输出目标可以是某个输入。
pub fn load(spec: &MergeSpec, options: &TableMergeOptions) -> Result<LoadedTableMerge, String> {
    for path in [&spec.left, &spec.right, &spec.center] {
        check_file(path)?;
    }
    let workbook = is_workbook(&spec.center);
    let inputs = if workbook {
        MergeInputs {
            left: load_workbook(&spec.left)?,
            center: load_workbook(&spec.center)?,
            right: load_workbook(&spec.right)?,
        }
    } else {
        let center = load_text(&spec.center, None)?;
        let header = Some(center.headers.is_some());
        MergeInputs {
            left: vec![load_text(&spec.left, header)?],
            right: vec![load_text(&spec.right, header)?],
            center: vec![center],
        }
    };
    let merge = TableMerge::new(inputs, options).map_err(|e| e.to_string())?;
    let packages = if workbook {
        let read = |path: &Path| {
            std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
        };
        Some([read(&spec.left)?, read(&spec.center)?, read(&spec.right)?])
    } else {
        None
    };
    let mut vba = None;
    if workbook {
        for path in [&spec.center, &spec.left, &spec.right] {
            if extension_is(path, &["xlsm"]) || !extension_is(path, &["xlsx"]) {
                if let Ok(Some(bytes)) = xlsx_write::vba_project(path) {
                    vba = Some(bytes);
                    break;
                }
            }
        }
    }
    let warnings = merge.warnings.clone();
    Ok(LoadedTableMerge {
        merge,
        workbook,
        vba,
        packages,
        warnings,
    })
}

/// 把合并结果写到 `path`（同目录临时文件再替换）。`sheets` = `loaded.merge.output()`。
/// 工作簿以 center 的包为模板写出（样式、列宽、合并单元格、图表、宏等保留，见
/// `bc_engine_table::xlsx_patch`）；模板写出失败时退回新写工作簿并给出提示。
/// 返回非致命提示。
pub fn write_output(
    sheets: &[Sheet],
    loaded: &LoadedTableMerge,
    path: &Path,
    inputs: &[&Path],
) -> Result<Vec<String>, String> {
    let mut warnings = Vec::new();
    let xlsx = extension_is(path, &["xlsx"]);
    let patched = loaded.packages.as_ref().map(|[l, c, r]| {
        bc_engine_table::xlsx_patch::patch_workbook(&loaded.merge, sheets, [l, c, r], !xlsx)
    });
    let bytes = if let Some(Ok((bytes, notes))) = patched {
        warnings.extend(notes);
        bytes
    } else if loaded.workbook {
        if let Some(Err(e)) = patched {
            warnings.push(format!("{e}; written without styles"));
        }
        let vba = loaded.vba.as_deref().filter(|_| !xlsx);
        if loaded.vba.is_some() && xlsx {
            warnings.push("macros not kept: the output is .xlsx".to_string());
        }
        if inputs.iter().any(|input| same_file(input, path)) {
            warnings
                .push("the workbook was rewritten without styles, charts or column widths".into());
        }
        xlsx_write::workbook_bytes(sheets, vba).map_err(|e| e.to_string())?
    } else {
        let mut out = Vec::new();
        if let Some(sheet) = sheets.first() {
            write_delimited(sheet, &mut out).map_err(|e| e.to_string())?;
        }
        out
    };
    crate::runner::replace_file(path, &bytes)?;
    Ok(warnings)
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// `/automerge` 的表格版：没有冲突 → 写出、0；有冲突 → 不写、101（表格没有冲突标记形式，
/// `/force` 由调用方事先拒绝）。`Err` = 输入不可用或写出失败（100）。
pub fn auto_merge_tables(
    spec: &MergeSpec,
    options: &TableMergeOptions,
) -> Result<AutoMergeReport, String> {
    let output: PathBuf = spec
        .output
        .clone()
        .ok_or_else(|| "/automerge needs an output file".to_string())?;
    let loaded = load(spec, options).map_err(|e| format!("/automerge: {e}"))?;
    let outcome = auto_merge(&loaded.merge);
    let mut warnings = loaded.warnings.clone();
    if !outcome.withheld {
        warnings.extend(write_output(
            &outcome.sheets,
            &loaded,
            &output,
            &[&spec.left, &spec.right, &spec.center],
        )?);
    }
    Ok(AutoMergeReport {
        code: if outcome.withheld {
            ExitCode::ConflictsMergeNotWritten
        } else {
            ExitCode::Success
        },
        conflicts: outcome.conflicts.len(),
        written: !outcome.withheld,
        warnings,
        keys: Some(loaded.merge.keys_label()),
    })
}

/// 引擎选项：`/keys=`，以及只属于 `/automerge`（和由它转入的界面）的 `/iu`、favor。
pub fn options(
    keys: Option<Vec<usize>>,
    ignore_unimportant: bool,
    favor: crate::cli::Favor,
) -> TableMergeOptions {
    TableMergeOptions {
        keys,
        ignore_unimportant,
        favor: match favor {
            crate::cli::Favor::None => bc_engine_table::merge::Favor::None,
            crate::cli::Favor::Left => bc_engine_table::merge::Favor::Left,
            crate::cli::Favor::Right => bc_engine_table::merge::Favor::Right,
        },
        ..Default::default()
    }
}

#[cfg(test)]
#[path = "table_merge_tests.rs"]
mod tests;
