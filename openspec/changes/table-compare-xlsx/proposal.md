# Proposal

## Why

表格比较是用户点名要"一比一再超越"的能力（FR-C1 及 proposal 路线图第四项）。参考工具的 Table Compare 本质是"把表格转成网格文本再按键对齐"：CSV 原生，xlsx 靠转换器只取值，不看公式与格式，多 sheet 需手动配对，键列要手动指定，列的插入/移动会整列飘红。`bc-engine-table` 目前只有骨架。本 change 先把参考工具的表格比较行为完整复刻，再在 xlsx 原生读取、sheet 自动配对、键列自动推断、列级对齐、值/公式/格式三态与按键差异摘要上超越它。

## What Changes

- 表格来源：分隔文本（分隔符/限定符/连续分隔符/环绕空白/自动检测）、固定列宽文本、首行表头检测与覆盖、区域设置（小数/千分位/日期顺序与分隔符）、编码覆盖；**xlsx/xlsm 原生读取**（多 sheet、值、公式、数字/日期类型、合并单元格范围），不再依赖外部转换器；保存分隔文本（xlsx 写回后置）。
- 比较列与 sheet：比较列映射（左右列号可不同、Move Up/Down、Tidy）、默认与逐列 Handling（Key、Use Default、Type General/Text、Unimportant、大小写/空白不重要、日期与数值容差）；sheet 按名配对与手动重排、"左文件为源"开关；**超越**：键列自动推断（唯一性+稳定性评分，可改）、按表头名与内容相似度自动对齐列并识别列插入/删除/重命名/移动、sheet 自动配对的名称归一化。
- 行对齐：仅对键匹配的行配对；Standard / Myers O(ND) / Patience / Unaligned 与 closeness 开关；排序后对齐；无键时用全部列做键。
- 单元格比较：按列类型与容差逐格比较，三态（相同/不重要/重要）；**超越**：值、公式、格式各自独立标记并可切换"只看值"。
- 视图：双网格同步滚动、行/单元格差异导航、Next/Previous Edit、显示过滤 All/Differences/Same/None、Ignore Unimportant、Hide Same Columns、Resize Columns、可见空白、行号、Side-by-Side/Over-Under、缩略图、Text Details 面板、File Info、sheet 标签页带颜色点、编辑（Modify/插入删除行列/排序/Copy to Other Side/Copy Row）、Align With、Find/Replace/Go To、两侧独立保存；**超越**：按键的结构化差异摘要导出（哪几行哪几列变了，文本/CSV）。
- 黄金语料：用参考工具脚本 `table-report`（`data-report`）生成期望，覆盖键对齐、排序、容差、不重要列、多 sheet（xlsx 由参考工具转换后比较，用于校验值级一致）。
- 不在本 change：xlsx 写回、Excel 公式求值、三路表格合并（随 `text-merge-and-ai`）、HTML tables 格式、报告输出（`hex-and-reports`）。

## Capabilities

### New Capabilities

- `table-source-formats`: 分隔/固定列宽/xlsx 来源的读取、表头与区域设置、编码、sheet 枚举、分隔文本保存。
- `table-columns-and-sheets`: 比较列映射与 Handling、键列定义与自动推断、列对齐与结构变化识别、sheet 配对。
- `table-row-alignment`: 按键的行配对、排序、对齐策略与 closeness、未配对行为孤儿。
- `table-cell-comparison`: 单元格按类型/容差/不重要规则的三态判定，值/公式/格式分层，行与 sheet 的状态汇总。
- `table-compare-view`: 双网格视图的用户可感知行为：导航、显示过滤、列操作、编辑、查找、保存、sheet 标签、差异摘要导出。

### Modified Capabilities

（无。表格格式的 Type/Regional 设置作为 `table-source-formats` 的需求定义，由现有格式库模型的 `TableOptions` 扩展承载；`text-compare-core` 的 `file-format-library` 需求不变。）

## Impact

- 代码：`crates/bc-engine-table`（全部）、`crates/bc-formats`（`TableOptions` 扩展为 Type/Regional，格式面板）、`crates/bc-gui`（新增 `table/` 模块与 `View::TableCompare`，Home 入口）、`crates/bc-core/tests/golden/table/*`。
- 依赖：新增 `calamine`（xlsx/xls/ods 读取，纯 Rust）；格式差异与写回所需的 `umya-spreadsheet` 延后到写回 change。均为引擎 crate 依赖，不含 GUI/异步运行时。
- 并行约束：在 master 上开发，与 `folder-compare-sync` 分支并行；`bc-gui/app.rs`、`home.rs` 只做分支与入口接线；`bc-core`/`bc-engine-text` 零修改（复用 `AlignMap`、`Aligner`、`CancelCheck`、`EditStack`）。
- 与 `text-compare-core` 共享：对齐策略公开名称、`DiffColors`、显示过滤语义、黄金语料流程与参考工具互斥锁。
