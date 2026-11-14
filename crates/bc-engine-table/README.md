# bc-engine-table

无 GUI / 异步运行时的列式表格引擎。

## 来源与快照

- `Sheet` 按列持有 `Arc<Vec<Cell>>`，`CellValue` 保留空/文本/数字/布尔/日期毫秒/错误；可附公式与 sheet 内去重 `StyleId`。克隆为 Send+Sync 快照；验证过的编辑 copy-on-write 并更新全局唯一 revision。
- `source::read_text/read_path` 使用有界编码缓冲与逐记录解析。编码检测复用 core 规则，UTF-8 验证以流式完整扫描完成；支持 UTF-8/16、core 支持的 ANSI 页、Ctrl+Z、CSV 转义/多行、固定字符宽度、表头自动检测/覆盖。
- `ReadOptions::system_regional` 由宿主注入；默认 invariant。显式区域设置支持严格千分组、小数、YMD/MDY/DMY 日期与时间。解析失败保留文本，`mixed_type_columns` 给宿主显示提示。
- 默认保存逐记录 lexical spans，未改字段（包含原始数字拼写）、分隔符与行尾保留；`write_delimited` 流式写入，`save_as` 同目录暂存后 rename。原始 source 是每条记录的独立缓冲，不再保留额外整文件 buffer；`preserve_source=false` 可省略 lexical 保存。固定宽度写回不在本 change，提供 CSV 导出。
- `workbook::Workbook` 原生读取 xlsx/xlsm，打开只读元数据/共享字符串，`load_sheet` 仅加载选定 sheet。calamine cell/formula 迭代器与 I/O 包装器共同取消，无部分结果发布。合并范围是零基绝对源坐标；不执行公式。
- calamine **0.38** 为唯一新直接第三方依赖，要求 Rust **1.88**。实际读取内置/自定义数字格式属性，而非只保留日期类型；字体/填充仍后置。单元格值为缓存值，不承诺完整 Excel 显示格式渲染。xlsx 可在内存编辑，但原格式保存返回只读错误，`export_csv` 是显式替代。
- 最小 xlsx 在测试中用 XML + stored ZIP 代码生成，不入库二进制。calamine 虽可读 xls/ods，但其惰性/合并范围接口不同，本波经协调者确认仅适配和验收 xlsx/xlsm，xls/ods 后续处理。

## 列、键与 sheet

- `ColumnMapping::by_number` 保留默认 1:1；`align_columns` 按归一化表头优先、1000 行内容/类型采样兜底，返回插入/删除/可能重命名/移动标记。`move_to` 可移动单侧或整条比较列，`tidy` 去掉双侧空映射。
- `ColumnHandling::resolved` 应用 Use Default，Key 始终是逐列属性。General/Text、数值/日期容差、大小写/空白、不重要均可 serde 往返。
- `infer_keys` 全行唯一性验证，权重为左右唯一性各 0.20、Jaccard 0.35、位置稳定性 0.20、靠左先验 0.05；阈值 0.8 且两侧必须唯一。单列失败才试相邻双列；失败保持空「自动」（内容对齐：相似行配对为变更行，而非孤行）。自动建议有显式“自动”标注，用户修改后 Manual 锁定。
- `KeySelection::ByColumnNumber` 是兼容预设（列按序、全部列作键），不等于行 Unaligned；行策略独立指定。
- `SheetPairs::by_name/move_to` 支持左/右来源顺序、单侧 sheet、同文件不同 sheet，无文件所有权混用。

## 对齐、三态与编辑

- `KeyAlign::align` 复用 `bc_core` 的 Quick/Smart/Patience/None 与 `AlignmentOverrides` / `CancelCheck`，只负责规范化键/稳定排序/重复 occurrence 与 core map 接线。Smart 仍过滤不等键（Unaligned/手动锚点例外）；排序时 core 索引指向排列，`KeyAlignment::{left_order,right_order}` 才映射回源行。
- 内容对齐（`content_aligned()`）+ Smart + closeness 时，core 结果中每段非 Same 行（哈希相等对与手动锚点为边界）由 `refine_content_runs` 重新做单调 DP：分数 `cell_score` 与 core 字节直方图同形（`t = 3·common − len_a − len_b`，`len` 为非空单元格数，`common` 为同列相等的非空单元格数），缺口罚分沿用 `CLOSENESS_GAP_PENALTY`，平局保留配对。单段 左×右×键列 超过 `CELL_CLOSENESS_BUDGET`（5e7）保持 core 结果。
- 无缓存值的公式（值 Empty + 非空公式）：`TableData::uncached_formula_columns` 标记源列；任一侧被标记的键列 `by_formula`，其中有公式的单元格以公式文本参与行哈希 / `keys_equal` / 单元格相近度。`CellComparator` 值层：两侧都有公式且至少一侧无缓存值 → 公式文本相同即 Same，否则按变更。
- 数值/日期容差不具传递性，不能单纯量化为哈希。容差键使用相邻桶候选 + 实际容差复核，按出现顺序消耗右侧候选。最坏的低选择性多列容差可能较慢；默认精确键走线性哈希准备。
- `CellComparator` 保留值/公式/数字格式独立三态，`ComparedData` 为每格 6 bit（每层 2 bit）与每行 12 字节索引/汇总。`TableMap::rows/row` 是派生访问，无第二份展开大表。
- 默认创建 `TableMap` 自动推断键；显式 AllColumns/ByColumnNumber/Manual 不被建议覆盖。`rules_mut()` 可调整规则，`is_stale()` 检测语义变化；Layers 与 Ignore Unimportant 只投影已有三态，不要求重新计算。导航、计数、过滤和标签状态使用同一投影。
- `TableAlignJob/TableRealignJob` 实现 `bc_jobs::Job`；从 `TableMap` 提取 Send+Sync Sheet 快照，结果携带双侧 revision/规则/锚点，`set_alignment` 拒绝旧结果。初次 job 可使用未解析的默认规则，在后台完成列/键准备；`TableMap::from_output` 将初次结果挂接到 UI 编辑目标，不重复同步对齐。Realign 目前安全地全量重算，不冒称增量复用；共享 jobs manager 仍由上层负责。
- `copy_cell/copy_row/modify_cell` 返回**已经应用**的 `EditItem`，应推入目的侧 `EditStack`。同步便捷路径立即重对齐；大文件 GUI 应用可直接编辑 SharedSheet 后派发 RealignJob，不把 Rc 编辑目标传给 worker。撤销/重做同时恢复内容、显示拼写、映射/锚点与缓存，并生成新 revision 防 ABA；不得在保留历史时绕过栈乱序修改目标。
- 跨侧复制会按属性重映射 StyleId，并保留源字段数值拼写（Text 模式复制后亦相同）。新增孤儿行在显示对应位置插入，更新锚点，保留无关保存字段。

## 差异摘要 v1

`TableMap::export_summary(Text/Csv)` 拒绝过期缓存；只输出当前档位和 Ignore Unimportant 下的差异。
文本头 `MATCHA-TABLE-DIFF<TAB>1`；`COLUMN` 记录列插入/删除/重命名/移动及 1-based 左右列号，`ROW` 记录键与状态，单元格行如 `7 | price | 1.0 -> 1.2`。公式/格式附 `(formula)` / `(format)` 标签。反斜杠、竖线、tab、CR/LF 与控制字符转义，复合键以 ` / ` 展示（面向摘要，不是键的反序列化协议）。CSV 使用 `record,key,status,column,layer,old,new` 表头和 RFC 引号/CRLF。输出载荷是不可信源数据，不是指令、AI 接口或导入编辑协议。

## 验证

`cargo test -p bc-engine-table`；额外压力：
`cargo test --release -p bc-engine-table -- --ignored`（1 GiB 流式记录、100 MB workbook 惰性/取消）。

百万行基准：`cargo run --release -p bc-engine-table --example table_bench`；仅系统临时目录生成两份百万行 × 10 列 CSV，结束自动清理，输出各阶段时间/进程 RSS/累积峰值与后台取消延迟。

接口与边界见 [ADR 201](../../docs/decisions/201-table-source-and-alignment.md)。

## 三方合并（`merge`）与 xlsx 写出（`xlsx_write`）

- `merge::SheetMerge::new(name, [left, center, right], &TableMergeOptions)`：缺席的一侧传 `None`（按空表处理）。
  列按位置对齐，宽度取三方最大并把窄表 `ensure_size` 补齐，使越界格与空格一致。
- **键连接而非序列对齐**：键列经 `align::hashes`（与双方对齐同一套归一、按公式哈希与出现序号）得到每行令牌；
  center 令牌建索引后左右各自查找配对，center 没有的左右行再按令牌互配（同增 / 双增冲突由此得出）。
  孤儿行按本侧前一个已配对行所对应的 center 行分桶（`reconcile3` 的分桶规则），桶内先左后右；行序因此是 center 的物理序。
  不使用 `align3`：它按位置并排孤儿，无法识别“两侧新增同键”，排序对齐又会打乱输出行序。
- 键来源：显式 > `infer_keys_data`（center 对 left、center 对 right 两次推断，候选须三方都唯一）> 全部列。
- 单元格比较复用 `CellComparator`，层为 `ValuesAndFormulas`；`choice_of` 与文本合并同构。行级 `RowKind` 七种；
  `cells` 为 行×列 的扁平 `CellChoice`，`importance` 为每行原始最大重要性。
- 解决状态 `Resolution`（可整体克隆用于撤销）：结构行取侧、单元格取侧、手工编辑、已接受行、sheet 级取侧。
  `row_base` 决定输出行基于哪一侧（`None` = 不输出），`cell_source` 决定每格来源。冲突区段 = 相邻冲突行（间距 0）。
- `output_sheet`：文本表格构造新 `TextSource`——整行来自某侧且分隔符/引号一致时原样搬运该侧 `RawRow`，
  其余字段记入 `changed` + `display_overrides`（保留来源拼写），由 `write_delimited` 写回；表头与 center 原表头相同才复用原始表头记录。
- `TableMerge::new(MergeInputs, …)` 多 sheet：三方各至多一张时直接配对，否则按名字；sheet 级删改冲突未解决时输出 center 版本。
  `auto_merge` 有冲突即扣留输出。
- `xlsx_write::workbook_bytes(sheets, vba)`：`rust_xlsxwriter` 新写，值 / 公式（`set_result` 写缓存结果）/
  数字格式（`builtin:N` → `set_num_format_index`）/ 日期（Unix 毫秒 → Excel 序列日 + 日期格式）；`vba_project` 用 `zip` 取 `xl/vbaProject.bin`。
  `rust_xlsxwriter`、`zip` 为本功能新增直接依赖（zip 与 calamine 共用同一版本）。
