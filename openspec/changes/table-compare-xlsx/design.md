# Design

## Context

动机见 `proposal.md - Why`。当前状态：

- `bc-engine-table` 只有骨架（`TableModel`、`TableRules`/`ColumnHandling`、`KeyAlign`、`TableMap`、两个 job 签名）。
- 可复用的已完成能力：`bc-core`（`Aligner` Quick/Smart/Patience/None 与 `AlignMap`、`AlignmentOverrides`、`CancelCheck`、`EditStack`、`LineHasher`）、`bc-engine-text`（编码检测/解码、`Realign` 的快照 + 修订号模式、`to_conflict_text` 的纯文本协议风格）、`bc-gui` 的正式文本控件（虚拟化、投影、导航条、worker 模式、设置面板样式）、`bc-formats` 的 `TableOptions`（当前只有分隔符与限定符）与格式管理器。
- `bc-gui/app.rs`、`home.rs` 同时被文件夹线（分支）改动，本 change 在 master 上进行，接线必须最小。
- 参考资料（design D9 全部扫过）：`commandstable.html`、`sessiontable*.html`、`dlgtablecolhandling.html`、`formattable*.html`、`table_working_with_multiple.html`、`viewtable.html`、`scripting_reference` 的 `table-report`。

## Goals / Non-Goals

**Goals:**

- CSV/固定宽度/xlsx 的表格比较可日常使用，行为与参考工具一致；xlsx 原生读取不走转换器。
- 四项超越：sheet 自动配对、键列自动推断、列结构变化识别、值/公式/格式三层；外加按键差异摘要导出。
- 键对齐百万行可用；GUI 虚拟化到行列两维。

**Non-Goals:**

- xlsx 写回与格式写回；公式求值；三路表格合并；HTML tables 格式；报告输出；外部转换程序（`Conversion` 的 External program 方式保留在模型里但不实现）。

## Decisions

### D1. 表格模型：列式存储 + 类型化单元格

- 选择：`TableModel` 每个 sheet 按列存储 `Vec<Cell>`，`Cell` 为 `enum { Empty, Text(Arc<str>), Number(f64), Bool, Date(i64 ms), Error(String) }` 加可选 `formula: Option<Arc<str>>` 与 `style: Option<StyleId>`（样式去重表）。行数百万时列式便于按列计算键哈希与相似度。
- 理由：键推断、列对齐与"只看值"切换都是按列的操作；去重样式避免每格存格式。
- 备选：行式 `Vec<Vec<String>>`（骨架现状）。类型信息丢失、内存大，放弃。

### D2. 来源读取分层

- 分隔/固定文本：自研解析器（RFC 4180 风格 + 参考工具的连续分隔符/环绕空白选项），编码走 `bc_core::encoding`。
- xlsx/xlsm/xls/ods：`calamine` 读取值、类型、公式文本（`Reader::worksheet_formula`）与合并范围；样式信息 `calamine` 不提供，格式层在本 change 内只对分隔文本返回"无格式"，对 xlsx 以 `calamine` 能给出的数字格式/日期判定为限，字体/填充格式差异作为 `StyleId` 预留并在报告列为后置（需要 `umya-spreadsheet` 时与写回一起引入）。
- 理由：`calamine` 纯 Rust、快、惰性按 sheet；避免一开始引入重型读写库。
- 风险：规格的"格式分层"场景在本 change 只能覆盖数字格式；记入未采纳项与 tasks 的验收说明。

### D3. 键列自动推断评分

- 选择：对每列 c 计算 `uniq_l(c)`、`uniq_r(c)`（唯一值比例）、`overlap(c)`（两侧值集合 Jaccard）、`stable(c)`（同值行的相对位置相关性）、`pos(c)`（越靠左越高）；得分 = 加权和，阈值 0.8；单列不达标时尝试相邻两列组合；结果标注"自动"，用户一旦编辑则固定。
- 理由：覆盖 id 列、复合键（日期+代码）两类常见情况，可解释、可调参。

### D4. 列对齐：表头名优先，内容相似度兜底

- 选择：先按归一化表头名精确匹配；未匹配列两两计算内容相似度（采样 1 000 行的值集合 Jaccard + 类型一致性），阈值 0.9 判为重命名；位置不同的同名列判为移动；其余为插入/删除。结果生成比较列映射，用户可改。
- 备选：只按列号（参考工具默认）。保留为"按列号"模式供用户切换，保证 1:1 兼容。

### D5. 行对齐复用 `bc-core::align`

- 选择：把每行键值（按键列 Handling 规范化后）哈希为 `ContentHash` 序列，交给 `Aligner`（Quick/Smart/Patience/None），Smart 的相似度回调用整行文本；`AlignmentOverrides` 承载 Align With。排序模式先按键稳定排序再对齐。
- 理由：与文本线共用策略名称、语义与黄金流程。

### D6. 单元格比较与三层

- 选择：`CellComparator` 按 `ColumnHandling` 规范化（大小写/空白/数值/日期容差）得到值层三态；公式层比较规范化后的公式文本；格式层比较 `StyleId` 解析出的属性；视图档位决定参与汇总的层。`TableMap` 行状态为紧凑 `u8` 位图（每列 2 bit 三态 × 3 层）。

### D7. GUI：新模块 + 虚拟化二维网格

- 选择：`bc-gui/src/table/{view,grid,header,sheet_tabs,details,settings_panel,find}.rs`；网格只渲染可见行列（`show_rows` + 自定义列窗口）；复用文本线的 worker/防抖/修订号模式；`app.rs` 只加 `View::TableCompare` 与 `HomeAction::OpenTableCompare`，`home.rs` 加表格卡片；颜色复用 `DiffColors`。
- 合并约束：与文件夹线的 `View::FolderCompare/FolderSync` 分支并存，不改对方代码。

### D8. 黄金语料

- 选择：用参考工具脚本 `load` + `table-report layout:side-by-side output-to:<html> output-options:html-color`（HTML 带状态 class，比纯文本可靠）生成期望，解析每行状态与差异单元格；xlsx 用例由参考工具自身转换后生成，我们只校验值层一致。会话/格式文件操作沿用文本线的备份-写-恢复与 `%TEMP%\matcha-reference-tool.lock` 互斥。

### D9. 参考工具行为对照（扫描结果）

扫描：`commandstable.html`、`sessiontable.html`、`sessiontablealignment.html`、`sessiontablecolumns.html`、`sessiontablesheets.html`、`sessiontableformats.html`、`sessiontablespecs.html`、`dlgtablecolhandling.html`、`formattable*.html`、`table_working_with_multiple.html`、`viewtable.html`。

### 未采纳项

| 参考工具项 | 处理 |
|---|---|
| Conversion 的 External program（Loading/Saving 命令行、`%s %t %o`） | 模型保留，不实现；xlsx 改为原生读取 |
| Open With > 关联程序 / Explorer | 后置到 `session-and-cli` |
| Next/Previous Difference Files、Copy File and Open Next（子会话） | 后置到文件夹线合并后的子会话机制 |
| Display Font / 字号命令 | 全局偏好，随 `session-and-cli` |
| Table Compare Report | 后置到 `hex-and-reports`（本 change 只用参考工具产出黄金） |
| HTML tables 格式 | 丢弃 |
| xlsx 字体/填充格式差异 | 后置（需要写回库），本 change 只做数字格式层 |

## Risks / Trade-offs

- [`calamine` 对公式/样式的覆盖有限] → 公式文本可得；样式仅数字格式；在报告与 README 明示，格式层接口预留。
- [键推断误判] → 结果标注"自动"并可一键改回"全部列/按列号"；黄金语料不依赖推断（显式指定键）。
- [百万行内存] → 列式 + `Arc<str>` 去重 + 2 bit 状态位图；基准在任务 3.5 验证。
- [与文件夹线合并冲突] → GUI 只加模块与分支；`prefs.rs` 不改。
- [区域设置解析歧义] → 严格按格式设置解析，失败回退文本比较并在状态区提示。

## Migration Plan

- 无数据迁移；Home/命令行按文件格式（`*.csv`/`*.xlsx` 掩码）进入表格视图；`/fv="Table Compare"` 随 `session-and-cli`。

## Open Questions

- 键推断的阈值与权重（语料调参，不影响接口）。
- 日期容差的单位选择（天/小时）是否取参考工具的默认，待对话框核实。
