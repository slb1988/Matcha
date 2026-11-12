//! `summary` 布局的表格差异报告（Matcha 自有文本格式，不对齐参考工具字节）。
//!
//! 目标读者是人和 AI：一眼可读的差异式文本，逐 sheet → 行 → 单元格列出差异，相同的行不出现。
//! 格式约定见 `docs/features/reports.md` 的“表格差异报告”一节；改格式必须同步该文档。
//! 两侧文件路径不写入报告：调用方命令行里已经带了，报告不再重复。
//!
//! 单元格内容是来自比较文件的不可信数据，这里只做转义保证结构可辨认，不赋予它任何指令含义。

use crate::{Layout, Renderer, Report, ReportError, ReportRequest};
use bc_core::DiffKind;
use bc_engine_table::{
    columns::ColumnChange, compare::Importance, map::TableMapRow, rules::Layers, store::SheetStore,
    TableMap,
};
use bc_session::SessionKind;
use std::fmt::Write as _;

/// 写在报告头部的图例，让读者（尤其 AI）不必查文档就能读懂行前缀。
pub const LEGEND: &str = "legend: \"~\" changed row, \"-\" left-only row, \"+\" right-only row; \
cell lines are \"column (ref): left -> right\"";

/// 一侧 sheet 的来源信息（报告里换算行号、单元格引用用）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SheetSide {
    pub name: String,
    /// 源数据之前被当作表头吃掉的行数（0 或 1）：源行号 = 行下标 + 1 + header_rows。
    pub header_rows: usize,
    /// 工作簿 sheet：行列下标是绝对坐标，可以给出 A1 引用。
    pub workbook: bool,
}

/// 一对 sheet 的比较结果。
pub enum SheetComparison {
    Compared {
        left: SheetSide,
        right: SheetSide,
        map: TableMap,
    },
    /// 只在一侧存在的 sheet：只报告规模，不展开内容。
    LeftOnly {
        name: String,
        rows: usize,
        columns: usize,
    },
    RightOnly {
        name: String,
        rows: usize,
        columns: usize,
    },
}

/// 表格报告渲染器：按配对顺序排列的 sheet 比较。
pub struct TableReportRenderer<'a> {
    pub sheets: &'a [SheetComparison],
}

impl Renderer for TableReportRenderer<'_> {
    fn render(&self, request: &ReportRequest) -> Result<Report, ReportError> {
        if request.layout != Layout::Summary {
            return Err(ReportError::UnsupportedLayout {
                kind: SessionKind::TableCompare,
                layout: request.layout,
            });
        }
        Ok(Report::Text(render(
            self.sheets,
            request.options.title_row,
        )))
    }
}

/// 一个 sheet 的总体状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Same,
    Unimportant,
    Different,
    LeftOnly,
    RightOnly,
}

impl State {
    fn label(self) -> &'static str {
        match self {
            State::Same => "same",
            State::Unimportant => "unimportant differences only",
            State::Different => "different",
            State::LeftOnly => "left-only",
            State::RightOnly => "right-only",
        }
    }
}

fn state_of(sheet: &SheetComparison) -> State {
    match sheet {
        SheetComparison::LeftOnly { .. } => State::LeftOnly,
        SheetComparison::RightOnly { .. } => State::RightOnly,
        SheetComparison::Compared { map, .. } => {
            let stats = map.statistics();
            match stats.importance() {
                Importance::Important => State::Different,
                _ if stats.structural_columns > 0 => State::Different,
                Importance::Unimportant => State::Unimportant,
                Importance::Same => State::Same,
            }
        }
    }
}

/// 渲染整份报告。`title_row`：见 [`crate::ReportOptions::title_row`]。
pub fn render(sheets: &[SheetComparison], title_row: Option<usize>) -> String {
    let states: Vec<State> = sheets.iter().map(state_of).collect();
    let count = |s: State| states.iter().filter(|&&x| x == s).count();
    let overall = if states
        .iter()
        .any(|s| matches!(s, State::Different | State::LeftOnly | State::RightOnly))
    {
        State::Different
    } else if states.contains(&State::Unimportant) {
        State::Unimportant
    } else {
        State::Same
    };
    let compared = states
        .iter()
        .filter(|s| !matches!(s, State::LeftOnly | State::RightOnly))
        .count();
    let mut out = String::new();
    let _ = writeln!(out, "result: {}", overall.label());
    let _ = writeln!(
        out,
        "sheets: {compared} compared, {} different, {} same, {} left-only, {} right-only",
        count(State::Different) + count(State::Unimportant),
        count(State::Same),
        count(State::LeftOnly),
        count(State::RightOnly),
    );
    let _ = writeln!(out, "{LEGEND}");
    for (sheet, state) in sheets.iter().zip(&states) {
        out.push('\n');
        match sheet {
            SheetComparison::LeftOnly {
                name,
                rows,
                columns,
            }
            | SheetComparison::RightOnly {
                name,
                rows,
                columns,
            } => {
                let _ = writeln!(
                    out,
                    "=== sheet {}: {} ({rows} rows x {columns} columns)",
                    quote_name(name),
                    state.label()
                );
            }
            SheetComparison::Compared { left, right, map } => {
                // A CSV/TSV "sheet" is just the file name, already in the header.
                let title = if !left.workbook && !right.workbook {
                    "table".to_string()
                } else if left.name == right.name {
                    format!("sheet {}", quote_name(&left.name))
                } else {
                    format!(
                        "sheet {} vs {}",
                        quote_name(&left.name),
                        quote_name(&right.name)
                    )
                };
                let _ = writeln!(out, "=== {title}: {}", state.label());
                render_sheet(&mut out, left, right, map, title_row);
            }
        }
    }
    out
}

/// sheet 名总是加引号，避免与后面的状态混淆。
fn quote_name(name: &str) -> String {
    format!("\"{}\"", escape(name, false).replace('"', "\\\""))
}

fn render_sheet(
    out: &mut String,
    ls: &SheetSide,
    rs: &SheetSide,
    map: &TableMap,
    title_row: Option<usize>,
) {
    let rules = map.rules();
    let left = map.left.borrow();
    let right = map.right.borrow();
    let columns = &rules.columns.0;
    // `title-row:<n>`：每个对齐列在标题行上的文字（左侧优先，左侧为空/无此列时取右侧）。
    let titles: Vec<String> = columns
        .iter()
        .map(|column| {
            let Some(n) = title_row else {
                return String::new();
            };
            let lt = column
                .left
                .and_then(|i| title_text(&left, ls, n, i))
                .unwrap_or_default();
            if !lt.is_empty() {
                return lt;
            }
            column
                .right
                .and_then(|i| title_text(&right, rs, n, i))
                .unwrap_or_default()
        })
        .collect();
    // 列名（不含标题）；第二项表示列名只是 Excel 列字母（工作簿且未识别表头）。
    let plain_name = |c: usize| -> (String, bool) {
        let column = &columns[c];
        let letters = |side: &SheetSide| side.workbook && side.header_rows == 0;
        column
            .left
            .map(|i| (column_name(&left, ls, i), letters(ls)))
            .or_else(|| {
                column
                    .right
                    .map(|i| (column_name(&right, rs, i), letters(rs)))
            })
            .unwrap_or_else(|| ((c + 1).to_string(), false))
    };
    let name_of = |c: usize| -> String {
        let (name, _) = plain_name(c);
        match titles[c].as_str() {
            "" => name,
            title if title == name => name,
            title => format!("{name}[{title}]"),
        }
    };

    let keys = rules.key_columns();
    let keyed = !keys.is_empty() && keys.len() < columns.len();
    if keyed {
        let names: Vec<String> = keys.iter().map(|&c| name_of(c)).collect();
        let _ = writeln!(out, "key: {}", names.join(", "));
    } else {
        let _ = writeln!(out, "key: (none; rows aligned by content)");
    }

    // 列结构变化。
    let mut structure = Vec::new();
    for (index, column) in columns.iter().enumerate() {
        if rules.ignore_unimportant && rules.handling(index).unimportant {
            continue;
        }
        for change in &column.changes {
            let lname = column.left.map(|c| column_name(&left, ls, c));
            let rname = column.right.map(|c| column_name(&right, rs, c));
            let place = column_place(column.left, column.right, ls, rs);
            let line = match change {
                ColumnChange::Inserted => format!(
                    "  + {} ({place}) inserted",
                    quote_name(rname.as_deref().unwrap_or_default())
                ),
                ColumnChange::Deleted => format!(
                    "  - {} ({place}) deleted",
                    quote_name(lname.as_deref().unwrap_or_default())
                ),
                ColumnChange::Renamed => format!(
                    "  ~ {} -> {} ({place}) renamed",
                    quote_name(lname.as_deref().unwrap_or_default()),
                    quote_name(rname.as_deref().unwrap_or_default())
                ),
                ColumnChange::Moved => format!(
                    "  ~ {} ({place}) moved",
                    quote_name(lname.or(rname).as_deref().unwrap_or_default())
                ),
            };
            structure.push(line);
        }
    }
    if !structure.is_empty() {
        let _ = writeln!(out, "columns:");
        for line in structure {
            let _ = writeln!(out, "{line}");
        }
    }

    let stats = map.statistics();
    let _ = writeln!(
        out,
        "rows: {} changed, {} unimportant, {} left-only, {} right-only, {} same",
        stats.important, stats.unimportant, stats.left_only, stats.right_only, stats.same
    );

    for (row_index, row) in map.rows().enumerate() {
        if row.importance == Importance::Same
            && row.kind != DiffKind::LeftOnly
            && row.kind != DiffKind::RightOnly
        {
            continue;
        }
        let key = if keyed {
            let parts: Vec<String> = keys
                .iter()
                .map(|&c| {
                    let column = &columns[c];
                    let value = row
                        .left
                        .and_then(|r| column.left.and_then(|i| left.display_at(r, i)))
                        .or_else(|| {
                            row.right
                                .and_then(|r| column.right.and_then(|i| right.display_at(r, i)))
                        })
                        .unwrap_or_default();
                    format!("{}={}", name_of(c), value_text(&value, false))
                })
                .collect();
            format!("{}  ", parts.join(", "))
        } else {
            String::new()
        };
        let place = row_place(&row, ls, rs);
        match row.kind {
            DiffKind::LeftOnly | DiffKind::RightOnly => {
                let (prefix, sheet, side_rows, index) = if row.kind == DiffKind::LeftOnly {
                    ("-", &left, row.left, Side::Left)
                } else {
                    ("+", &right, row.right, Side::Right)
                };
                let _ = writeln!(out, "{prefix} {key}({place})");
                let Some(r) = side_rows else { continue };
                let cells: Vec<String> = columns
                    .iter()
                    .enumerate()
                    .filter_map(|(c, column)| {
                        let i = match index {
                            Side::Left => column.left,
                            Side::Right => column.right,
                        }?;
                        let value = sheet.display_at(r, i).unwrap_or_default();
                        Some(format!("{}={}", name_of(c), value_text(&value, true)))
                    })
                    .collect();
                let _ = writeln!(out, "    {}", cells.join(" | "));
            }
            _ => {
                let note = if row.importance == Importance::Unimportant {
                    " (unimportant)"
                } else {
                    ""
                };
                let _ = writeln!(out, "~ {key}({place}){note}");
                for (c, column) in columns.iter().enumerate() {
                    let Some(diff) = map.cell_diff(row_index, c) else {
                        continue;
                    };
                    let lc = row
                        .left
                        .and_then(|r| column.left.and_then(|i| left.cell_snapshot(r, i)));
                    let rc = row
                        .right
                        .and_then(|r| column.right.and_then(|i| right.cell_snapshot(r, i)));
                    for (layer, importance) in [
                        ("value", diff.value),
                        ("formula", diff.formula),
                        ("format", diff.format),
                    ] {
                        let importance = importance.projected(rules.ignore_unimportant);
                        if importance == Importance::Same
                            || (layer == "formula" && rules.layers == Layers::Values)
                            || (layer == "format" && rules.layers != Layers::All)
                        {
                            continue;
                        }
                        let (old, new) = match layer {
                            "formula" => (
                                lc.as_ref()
                                    .and_then(|c| c.formula.as_deref())
                                    .unwrap_or("")
                                    .to_string(),
                                rc.as_ref()
                                    .and_then(|c| c.formula.as_deref())
                                    .unwrap_or("")
                                    .to_string(),
                            ),
                            "format" => (
                                left.style(lc.as_ref().and_then(|c| c.style))
                                    .and_then(|s| s.number_format.as_deref())
                                    .unwrap_or("")
                                    .to_string(),
                                right
                                    .style(rc.as_ref().and_then(|c| c.style))
                                    .and_then(|s| s.number_format.as_deref())
                                    .unwrap_or("")
                                    .to_string(),
                            ),
                            _ => (
                                row.left
                                    .and_then(|r| column.left.and_then(|i| left.display_at(r, i)))
                                    .unwrap_or_default(),
                                row.right
                                    .and_then(|r| column.right.and_then(|i| right.display_at(r, i)))
                                    .unwrap_or_default(),
                            ),
                        };
                        // An inserted/deleted column has no cell on one side at all.
                        let side_text = |present: bool, text: &str| {
                            if present {
                                value_text(text, false)
                            } else {
                                "(no column)".to_string()
                            }
                        };
                        let (old, new) = (
                            side_text(column.left.is_some(), &old),
                            side_text(column.right.is_some(), &new),
                        );
                        let cell = cell_ref(row.left, row.right, column.left, column.right, ls, rs);
                        // 列名只是字母且有标题时，单元格引用里已带列字母：只写标题，
                        // `AttachConfigs (K1172)` 而不是 `K[AttachConfigs] (K1172)`。
                        let name = match (plain_name(c), titles[c].as_str()) {
                            ((_, true), title) if !title.is_empty() && cell.is_some() => {
                                title.to_string()
                            }
                            _ => name_of(c),
                        };
                        let label = if layer == "value" {
                            name
                        } else {
                            format!("{name} {layer}")
                        };
                        let reference = cell.map(|r| format!(" ({r})")).unwrap_or_default();
                        let note = if importance == Importance::Unimportant
                            && row.importance != Importance::Unimportant
                        {
                            "  (unimportant)"
                        } else {
                            ""
                        };
                        let _ = writeln!(
                            out,
                            "    {}{reference}: {old} -> {new}{note}",
                            escape(&label, false),
                        );
                    }
                }
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Side {
    Left,
    Right,
}

/// 列名：有表头用表头；无表头的工作簿用列字母，其他用 1 起列号。
fn column_name(sheet: &SheetStore, side: &SheetSide, column: usize) -> String {
    if side.header_rows == 0 && side.workbook {
        column_letters(column)
    } else {
        sheet.column_name(column)
    }
}

/// 标题行 `n`（1 起源行号）在第 `column` 列的文字；连续空白（含换行、制表）压成一个空格，
/// 保证标题单行（单元格行里列名还会整体转义一次，这里不预先转义）。
/// 该行已被当作表头吃掉（`n <= header_rows`）或超出数据范围时为 `None`：前者列名本身就是标题。
fn title_text(sheet: &SheetStore, side: &SheetSide, n: usize, column: usize) -> Option<String> {
    let row = n.checked_sub(1 + side.header_rows)?;
    if row >= sheet.row_count() {
        return None;
    }
    let text = sheet.display_at(row, column)?;
    Some(text.split_whitespace().collect::<Vec<_>>().join(" "))
}

/// 0 起列下标 → Excel 列字母（0 → A，26 → AA）。
pub fn column_letters(mut column: usize) -> String {
    let mut letters = Vec::new();
    loop {
        letters.push(b'A' + (column % 26) as u8);
        if column < 26 {
            break;
        }
        column = column / 26 - 1;
    }
    letters.reverse();
    String::from_utf8(letters).unwrap()
}

/// 源行号（1 起，含表头行）。
fn source_row(side: &SheetSide, row: usize) -> usize {
    row + 1 + side.header_rows
}

fn row_place(row: &TableMapRow, ls: &SheetSide, rs: &SheetSide) -> String {
    match (row.left, row.right) {
        (Some(l), Some(r)) => format!(
            "left row {}, right row {}",
            source_row(ls, l),
            source_row(rs, r)
        ),
        (Some(l), None) => format!("left row {}", source_row(ls, l)),
        (None, Some(r)) => format!("right row {}", source_row(rs, r)),
        (None, None) => String::new(),
    }
}

/// 列结构变化的位置描述：工作簿用列字母，其他用 1 起列号。
fn column_place(
    left: Option<usize>,
    right: Option<usize>,
    ls: &SheetSide,
    rs: &SheetSide,
) -> String {
    let at = |side: &SheetSide, c: usize| {
        if side.workbook {
            column_letters(c)
        } else {
            format!("#{}", c + 1)
        }
    };
    match (left, right) {
        (Some(l), Some(r)) => format!("left {}, right {}", at(ls, l), at(rs, r)),
        (Some(l), None) => format!("left {}", at(ls, l)),
        (None, Some(r)) => format!("right {}", at(rs, r)),
        (None, None) => String::new(),
    }
}

/// 工作簿单元格的 A1 引用；两侧相同时只写一个。非工作簿没有引用。
fn cell_ref(
    lrow: Option<usize>,
    rrow: Option<usize>,
    lcol: Option<usize>,
    rcol: Option<usize>,
    ls: &SheetSide,
    rs: &SheetSide,
) -> Option<String> {
    let a1 = |side: &SheetSide, r: Option<usize>, c: Option<usize>| {
        Some(format!("{}{}", column_letters(c?), source_row(side, r?)))
    };
    let l = ls.workbook.then(|| a1(ls, lrow, lcol)).flatten();
    let r = rs.workbook.then(|| a1(rs, rrow, rcol)).flatten();
    match (l, r) {
        (Some(l), Some(r)) if l == r => Some(l),
        (Some(l), Some(r)) => Some(format!("left {l}, right {r}")),
        (Some(l), None) => Some(format!("left {l}")),
        (None, Some(r)) => Some(format!("right {r}")),
        (None, None) => None,
    }
}

/// 转义反斜杠、换行、制表与控制字符；`pipe` 时把 `|` 写成 `\|`（单侧行的分隔符）。
fn escape(text: &str, pipe: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '|' if pipe => out.push_str("\\|"),
            c if c.is_control() => {
                let _ = write!(out, "\\u{{{:x}}}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

/// 报告中的单元格值：空值写 `(empty)`；首尾带空白、本身就是 `(empty)` 或含 ` -> ` 时加引号。
fn value_text(text: &str, pipe: bool) -> String {
    if text.is_empty() {
        return "(empty)".into();
    }
    let escaped = escape(text, pipe);
    let needs_quotes = text.starts_with(char::is_whitespace)
        || text.ends_with(char::is_whitespace)
        || text == "(empty)"
        || text.contains(" -> ")
        || text.starts_with('"');
    if needs_quotes {
        format!("\"{}\"", escaped.replace('"', "\\\""))
    } else {
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters() {
        assert_eq!(column_letters(0), "A");
        assert_eq!(column_letters(25), "Z");
        assert_eq!(column_letters(26), "AA");
        assert_eq!(column_letters(51), "AZ");
        assert_eq!(column_letters(52), "BA");
        assert_eq!(column_letters(701), "ZZ");
        assert_eq!(column_letters(702), "AAA");
    }

    #[test]
    fn values_are_unambiguous() {
        assert_eq!(value_text("", false), "(empty)");
        assert_eq!(value_text("a b", false), "a b");
        assert_eq!(value_text(" a", false), "\" a\"");
        assert_eq!(value_text("(empty)", false), "\"(empty)\"");
        assert_eq!(value_text("x -> y", false), "\"x -> y\"");
        assert_eq!(value_text("l1\nl2", false), "l1\\nl2");
        assert_eq!(value_text("a|b", true), "a\\|b");
        assert_eq!(value_text("a|b", false), "a|b");
        assert_eq!(value_text("c:\\x", false), "c:\\\\x");
    }
}
