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
- Text Details 对照条（网格下方，当前行两侧各一行）：
  - 列宽为**独立覆盖值**：拖拽对照条列头边界只改对照条（±3 px 命中、最小 20 px），
    双击边界按当前行两侧内容 + 列头适配（≤1000 px）；未调整的列跟随上方网格列宽。
    `Resize Columns to Fit`、切换 sheet、重新加载、列数变化时清除全部覆盖。
  - 独立水平滚动：内容超宽时底部显示横向滚动条（像素级，否则保留原面板色带），鼠标在对照条上滚轮只滚对照条；
    光标列变化时以最小滚动量使其可见（超宽列左对齐），手动滚开后不会被吸回。
  - 单击某侧单元格 = 在上方网格点击该侧当前行该列（切活动侧、键盘焦点交给该侧网格、
    网格横向滚到该格）；Shift+单击在当前行扩展选区；单击列头移到活动侧该列；
    双击 = Modify；右键先按网格规则选中再弹出同一表格上下文菜单；Align With 待选时同样生效。
    上方选区覆盖当前行的格子在对照条活动侧以选中色显示。
  - 悬停被截断或含换行的单元格时提示完整原文（保留换行，最多 4000 字符）。
- xlsx/xlsm：calamine 0.38 只读打开（元数据/共享字符串），按需 `load_sheet`；
  内存可编辑，原格式保存返回只读错误，`export_csv` 为显式替代。
- 保存保真：逐记录 lexical spans，未改字段、分隔符、行尾原样保留；同目录暂存后 rename。
- 键对齐比较（`KeyAlign`）、表格规则（`TableRules`）、混合类型列提示。
- 界面：多 sheet 工作簿在网格与状态栏之间显示 sheet 标签（状态点：灰 = 尚未比较、弱化色 = 相同、差异色 = 有差异）；
  网格右键菜单按参考排列（Align With、Copy to Left/Right、Cut/Copy/Paste、插删行列、Modify…、Open/Save、Explorer），
  右键先选中所点格；Session Settings 为 Specs / Format / Columns / Alignment / Sheets 五页，OK / Cancel / Apply。
  工作簿一侧的信息行不显示分隔符。
- **打开即定位首个差异格**：打开文件对、切到另一个 sheet 标签或在会话里换一侧文件后，比较首次就绪时
  光标与滚动自动落到第一个差异行的第一个改动格（与 Next Difference 同口径；整行只有行序差异时落在首个可见列）；
  无差异则停在左上角。一次性（`TableCompareView::land_on_first_diff`）：就绪前已移动光标或编辑则不跳，
  编辑后的重新对齐、Recompare、显示层切换不会把光标拉回。
- 入口路由：两个文件按 `bc_formats::route` 统一判定（见 [gui-shell.md](gui-shell.md)「文件对的会话类型统一路由」），
  xlsx/xlsm/csv/tsv/tab 与用户表格格式默认打开表格比较，先于二进制嗅探（xlsx 是 ZIP，此前会误进十六进制）；
  文件夹比较里打开这类文件对时新开表格比较标签页。`/qc` 规则比较对表格按单元格比较（见 cli-and-scripting.md）。
- 命令行：`matcha /report=summary a.xlsx b.xlsx`（xlsx 用 `/report=patch…` 也会自动改走）输出
  按 sheet / 行 / 单元格列出的文本差异报告，格式见 [reports.md](reports.md#表格差异报告table-report-layoutsummary)。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 引擎 | `crates/bc-engine-table` | `Sheet`/`Cell`、`source`（读入）、`workbook`（xlsx）、`KeyAlign`、`TableRules`、写回/导出 |
| CLI 报告 | `crates/bc-script/src/table_report.rs`、`crates/bc-report/src/table.rs` | 无窗口读取/按 sheet 比较 + 文本渲染 |
| 路由 | `crates/bc-formats/src/route.rs` | 表格 → 十六进制 → 文本的唯一判定（`is_workbook`/`is_table`/`pair_view`） |
| GUI | `crates/bc-gui/src/table/` | `details.rs`（Text Details 对照条绘制+交互，产出 `GridAction`；缩略图）、`view.rs`、`grid.rs`、`worker.rs`、`sorting.rs`、`find.rs`、`sheet_tabs.rs`、`settings_panel.rs` 等（含大量 `*_tests.rs`） |

## 关键语义

- 引擎无 GUI、无异步运行时；解析失败保留文本而不是报错中断。
- 键选择与行对齐：键列推断失败时保持空「自动」（内容对齐，相似行配对为变更行）；只有显式「全部列」/「按列号」/手动键才把任意单元格变更视为整行不同（孤行）。对齐校验拆分（verify filter）仅作用于真实键选择。
- 内容对齐（无键）的差异块内按「单元格相近度」配对：逐列比较非空单元格是否一致（不是整行文本的
  字节直方图），稀疏行会配给与它共享单元格的那一行；单块 左行×右行×列 超过 5000 万次比较时沿用文本相近度。
- 不执行公式；单元格值为缓存值，不承诺完整 Excel 显示格式渲染。公式**无缓存值**（不重算的工具保存，
  `<f>…</f><v/>`）时值视为未知：两侧都有公式且至少一侧无缓存值 → 按公式文本判定值是否一致；
  行对齐哈希在含此类单元格的列上也按公式文本。显示仍为空（不伪造计算结果）；同公式不同输入导致的结果
  变化因此无法检出。参考工具（BC 5，2026-11-13 实测）不调用 Excel，用内置求值器：工作簿声明
  `<calcPr fullCalcOnLoad="1"/>` 时两侧全部重算（连过期缓存值也替换），否则读缓存值、无缓存值的公式显示为 0。
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
- 三方合并（复用本功能的读取、键哈希、键推断与单元格比较）：[table-merge.md](table-merge.md)
- 实施报告：`docs/worker-reports/table-compare-xlsx-wave1…7*.md`、`table-memory-handoff.md`
