//! `table-report`（`/report=summary`，以及工作簿上的 `/report=patch…`）的无窗口执行：
//! 读两侧表格 → 按 sheet 配对 → 每对用表格引擎的默认规则比较 → 交给
//! [`bc_report::table::TableReportRenderer`]。
//!
//! 读取规则与 GUI 表格视图一致（`bc-gui/src/table/formats.rs`、`worker.rs`）：xlsx/xlsm 原生
//! 读取；其他文件按文件格式库的表格格式读取，没有格式时 `.tsv` 用制表符、其余自动探测分隔符；
//! 表头按格式设置（默认自动检测）。两侧都只有一个 sheet 时不论名字都配对，否则按名字配对。

use bc_core::cancel::{CancelCheck, NeverCancel};
use bc_engine_table::{
    sheets::{SheetPair, SheetPairs},
    source::{detect_header, read_path, ReadOptions},
    workbook::Workbook,
    Sheet, TableMap, TableRules,
};
use bc_formats::model::{ConvertKind, HeaderMode};
use bc_formats::registry::FormatRegistry;
use bc_formats::FormatKind;
use bc_report::table::{SheetComparison, SheetSide};
use std::path::Path;
use std::sync::Arc;

/// 读取 / 比较失败：`load` 为真时是文件读不出来（退出码 107），否则为其他错误（100）。
#[derive(Debug)]
pub struct TableError {
    pub load: bool,
    pub message: String,
}

impl TableError {
    fn load(message: String) -> Self {
        Self {
            load: true,
            message,
        }
    }
    fn other(message: String) -> Self {
        Self {
            load: false,
            message,
        }
    }
}

fn has_extension(path: &Path, names: &[&str]) -> bool {
    path.extension()
        .is_some_and(|e| names.iter().any(|name| e.eq_ignore_ascii_case(name)))
}

/// 原生读取的工作簿（xlsx/xlsm，含扩展名对不上的 xlsx 包）；判定见 [`bc_formats::route`]。
pub fn is_workbook(path: &Path) -> bool {
    bc_formats::route::is_workbook(path)
}

/// 表格读取选项（GUI 表格比较与脚本报告/合并共用这一份）。
pub fn read_options(path: &Path, registry: &FormatRegistry) -> Result<ReadOptions, TableError> {
    if has_extension(path, &["xls", "ods"]) {
        return Err(TableError::other(format!(
            "{}: this workbook type is not supported; use XLSX/XLSM or CSV",
            path.display()
        )));
    }
    let mut options = ReadOptions::default();
    if let Some(format) = registry.resolve(path, Some(FormatKind::Table)) {
        if format
            .convert
            .is_some_and(|c| c != ConvertKind::MsExcel || !is_workbook(path))
        {
            return Err(TableError::other(format!(
                "{}: table formats with external conversion are not supported",
                path.display()
            )));
        }
        options.format = format.table.clone().unwrap_or_default();
    } else if has_extension(path, &["tsv"]) {
        options.format.delimiters = vec!['\t'];
    } else {
        options.format.detect_delimiter = true;
    }
    Ok(options)
}

/// 一侧已打开的表格源。
struct Source {
    path: std::path::PathBuf,
    options: ReadOptions,
    names: Vec<String>,
    /// 非工作簿只有一张表，打开时就已读完。
    text: Option<Sheet>,
}

impl Source {
    fn open(path: &Path, registry: &FormatRegistry) -> Result<Self, TableError> {
        let options = read_options(path, registry)?;
        let cannot = |e: &dyn std::fmt::Display| {
            TableError::load(format!("cannot read {}: {e}", path.display()))
        };
        if is_workbook(path) {
            let book = Workbook::open(path, cancel()).map_err(|e| cannot(&e))?;
            Ok(Self {
                path: path.to_path_buf(),
                options,
                names: book.sheet_names().to_vec(),
                text: None,
            })
        } else {
            let sheet = read_path(path, &options, &NeverCancel)
                .map_err(|e| cannot(&e))?
                .sheet;
            Ok(Self {
                path: path.to_path_buf(),
                options,
                names: vec![sheet.name.clone()],
                text: Some(sheet),
            })
        }
    }

    fn workbook(&self) -> bool {
        self.text.is_none()
    }

    /// 第 `index` 张 sheet 与吃掉的表头行数。
    fn sheet(&self, index: usize) -> Result<(Sheet, usize), TableError> {
        if let Some(sheet) = &self.text {
            let header = usize::from(sheet.headers.is_some());
            return Ok((sheet.clone(), header));
        }
        let name = &self.names[index];
        let mut sheet = bc_engine_table::workbook::load_sheet_parallel(&self.path, name, cancel())
            .map_err(|e| {
                TableError::load(format!(
                    "cannot read sheet {name:?} of {}: {e}",
                    self.path.display()
                ))
            })?;
        let format = &self.options.format;
        sheet.regional = if format.regional.use_system {
            self.options.system_regional.clone()
        } else {
            format.regional.clone()
        };
        let header = self.options.header.unwrap_or(format.header);
        let take =
            header == HeaderMode::Names || (header == HeaderMode::Auto && detect_header(&sheet));
        if take {
            sheet.take_header();
        }
        Ok((sheet, usize::from(take)))
    }

    fn side(&self, index: usize, header_rows: usize) -> SheetSide {
        SheetSide {
            name: self.names[index].clone(),
            header_rows,
            workbook: self.workbook(),
        }
    }
}

fn cancel() -> Arc<dyn CancelCheck> {
    Arc::new(NeverCancel)
}

/// 读取并比较两侧的全部 sheet，按配对顺序返回。
pub fn compare(
    left: &Path,
    right: &Path,
    registry: &FormatRegistry,
    ignore_unimportant: bool,
) -> Result<Vec<SheetComparison>, TableError> {
    let sources = [
        Source::open(left, registry)?,
        Source::open(right, registry)?,
    ];
    let pairs = if sources[0].names.len() == 1 && sources[1].names.len() == 1 {
        SheetPairs(vec![SheetPair {
            left: Some(0),
            right: Some(0),
        }])
    } else {
        SheetPairs::by_name(&sources[0].names, &sources[1].names, true)
    };
    let mut out = Vec::with_capacity(pairs.0.len());
    for pair in pairs.0 {
        match (pair.left, pair.right) {
            (Some(l), Some(r)) => {
                let (lsheet, lheader) = sources[0].sheet(l)?;
                let (rsheet, rheader) = sources[1].sheet(r)?;
                let rules = TableRules {
                    ignore_unimportant,
                    ..TableRules::default()
                };
                let map = TableMap::new(lsheet, rsheet, rules, &NeverCancel).map_err(|e| {
                    TableError::other(format!(
                        "comparing sheet {:?} failed: {e}",
                        sources[0].names[l]
                    ))
                })?;
                out.push(SheetComparison::Compared {
                    left: sources[0].side(l, lheader),
                    right: sources[1].side(r, rheader),
                    map,
                });
            }
            (Some(l), None) => {
                let (sheet, _) = sources[0].sheet(l)?;
                out.push(SheetComparison::LeftOnly {
                    name: sources[0].names[l].clone(),
                    rows: sheet.row_count(),
                    columns: sheet.column_count(),
                });
            }
            (None, Some(r)) => {
                let (sheet, _) = sources[1].sheet(r)?;
                out.push(SheetComparison::RightOnly {
                    name: sources[1].names[r].clone(),
                    rows: sheet.row_count(),
                    columns: sheet.column_count(),
                });
            }
            (None, None) => {}
        }
    }
    Ok(out)
}
