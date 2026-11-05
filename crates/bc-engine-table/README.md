# bc-engine-table

无 GUI / 异步运行时的列式表格引擎。

## 来源与快照

- `Sheet` 按列持有 `Arc<Vec<Cell>>`，`CellValue` 保留空/文本/数字/布尔/日期毫秒/错误；可附公式与 sheet 内去重 `StyleId`。克隆为 Send+Sync 快照；验证过的编辑 copy-on-write 并更新全局唯一 revision。
- `source::read_text/read_path` 使用有界编码缓冲与逐记录解析。编码检测复用 core 规则，UTF-8 验证以流式完整扫描完成；支持 UTF-8/16、core 支持的 ANSI 页、Ctrl+Z、CSV 转义/多行、固定字符宽度、表头自动检测/覆盖。
- `ReadOptions::system_regional` 由宿主注入；默认 invariant。显式区域设置支持严格千分组、小数、YMD/MDY/DMY 日期与时间。解析失败保留文本，`mixed_type_columns` 给宿主显示提示。
- 默认保存逐记录 lexical spans，未改字段（包含原始数字拼写）、分隔符与行尾保留；`write_delimited` 流式写入，`save_as` 同目录暂存后 rename。原始 source 是每条记录的独立缓冲，不再保留额外整文件 buffer；`preserve_source=false` 可省略 lexical 保存。固定宽度写回不在本 change，提供 CSV 导出。
- `workbook::Workbook` 原生读取 xlsx/xlsm，打开只读元数据/共享字符串，`load_sheet` 仅加载选定 sheet。calamine cell/formula 迭代器与 I/O 包装器共同取消，无部分结果发布。合并范围是零基绝对源坐标；不执行公式。
- calamine **0.38** 为唯一新直接第三方依赖，要求 Rust **1.88**。实际读取内置/自定义数字格式属性，而非只保留日期类型；字体/填充仍后置。单元格值为缓存值，不承诺完整 Excel 显示格式渲染。xlsx 可在内存编辑，但原格式保存返回只读错误，`export_csv` 是显式替代。
- 最小 xlsx 在测试中用 XML + stored ZIP 代码生成，不入库二进制。

## 列、键与 sheet

- `ColumnMapping::by_number` 保留默认 1:1；`align_columns` 按归一化表头优先、1000 行内容/类型采样兜底，返回插入/删除/可能重命名/移动标记。`move_to` 可移动单侧或整条比较列，`tidy` 去掉双侧空映射。
- `ColumnHandling::resolved` 应用 Use Default，Key 始终是逐列属性。General/Text、数值/日期容差、大小写/空白、不重要均可 serde 往返。
- `infer_keys` 全行唯一性验证，权重为左右唯一性各 0.20、Jaccard 0.35、位置稳定性 0.20、靠左先验 0.05；阈值 0.8 且两侧必须唯一。单列失败才试相邻双列；失败为全部列。自动建议有显式“自动”标注，用户修改后 Manual 锁定。
- `KeySelection::ByColumnNumber` 是兼容预设（列按序、全部列作键），不等于行 Unaligned；行策略独立指定。
- `SheetPairs::by_name/move_to` 支持左/右来源顺序、单侧 sheet、同文件不同 sheet，无文件所有权混用。

## 验证

`cargo test -p bc-engine-table`；额外压力：
`cargo test --release -p bc-engine-table -- --ignored`（1 GiB 流式记录、100 MB workbook 惰性/取消）。

接口与边界见 [ADR 201](../../docs/decisions/201-table-source-and-alignment.md)。
