# 表格比较（Table Compare：CSV / XLSX）

> 列式表格引擎 + 表格比较视图：CSV/TSV/固定宽度与原生 xlsx/xlsm。状态：**已实现**
> （openspec `table-compare-xlsx` 23/23）。对应 FR-C1。

## 功能简介

- `Sheet` 按列持有 `Arc<Vec<Cell>>`；`CellValue` 保留空/文本/数字/布尔/日期毫秒/错误，
  附公式与去重 StyleId；克隆即 Send+Sync 快照。
- 文本来源：有界编码缓冲 + 逐记录解析；UTF-8/16、ANSI 页、CSV 转义/多行、固定宽度、
  表头自动检测/覆盖；区域设置（千分组/小数/日期顺序）由宿主注入。
- 网格列头固定为 Excel 风格字母（A、B、C…）：GUI 默认把首行当普通数据行显示，
  与参考工具一致；仅显式选择 `Column names`（格式设置）才把首行消费为列名
  （名称供按名对齐/报告使用，列头仍显示字母）。表头自动检测继续服务于 CLI 报告路径。
- 列宽：初始按内容适配（7 px/字符 + 12 px，`Resize Columns to Fit` 全量重算）；
  拖拽任一侧表头的列边界可手动调宽（两侧共享，最小 20 px，悬停出现水平调整光标）。
- xlsx/xlsm：calamine 0.38 只读打开（元数据/共享字符串），按需 `load_sheet`；
  内存可编辑，原格式保存返回只读错误，`export_csv` 为显式替代。
- 保存保真：逐记录 lexical spans，未改字段、分隔符、行尾原样保留；同目录暂存后 rename。
- 键对齐比较（`KeyAlign`）、表格规则（`TableRules`）、混合类型列提示。
- 命令行：`matcha /report=summary a.xlsx b.xlsx`（xlsx 用 `/report=patch…` 也会自动改走）输出
  按 sheet / 行 / 单元格列出的文本差异报告，格式见 [reports.md](reports.md#表格差异报告table-report-layoutsummary)。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-engine-table` | `Sheet`/`Cell`、`source`（读入）、`workbook`（xlsx）、`KeyAlign`、`TableRules`、写回/导出 |
| CLI 报告 | `crates/bc-script/src/table_report.rs`、`crates/bc-report/src/table.rs` | 无窗口读取/按 sheet 比较 + 文本渲染 |
| GUI | `crates/bc-gui/src/table/` | `view.rs`、`grid.rs`、`worker.rs`、`sorting.rs`、`find.rs`、`sheet_tabs.rs`、`settings_panel.rs` 等（含大量 `*_tests.rs`） |

## 关键语义

- 引擎无 GUI、无异步运行时；解析失败保留文本而不是报错中断。
- 键选择与行对齐：键列推断失败时保持空「自动」（内容对齐，相似行配对为变更行）；只有显式「全部列」/「按列号」/手动键才把任意单元格变更视为整行不同（孤行）。对齐校验拆分（verify filter）仅作用于真实键选择。
- 不执行公式；单元格值为缓存值，不承诺完整 Excel 显示格式渲染。
- 合并范围是零基绝对源坐标。
- calamine 0.38 是工作区 Rust ≥1.88 的原因。
- 系统文件拖放：拖到哪一侧（窗格、路径栏、列头均可）就替换哪一侧；规则见 [gui-shell.md](gui-shell.md)「系统文件拖放落点」。

## 使用与验证

```powershell
cargo test -p bc-engine-table
cargo test -p bc-gui --lib          # 表格视图测试在 bc-gui
matcha /report=summary left.xlsx right.xlsx   # 命令行差异报告
```

## 深入阅读

- `crates/bc-engine-table/README.md`（中文，最详尽的实现说明）
- ADR：`docs/decisions/201-table-source-and-alignment.md`、`202-table-view-and-golden.md`
- 行为对齐：`docs/reference-parity/table-compare.md`
- 实施报告：`docs/worker-reports/table-compare-xlsx-wave1…7*.md`、`table-memory-handoff.md`
