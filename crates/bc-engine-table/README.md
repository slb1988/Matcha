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

## 验证

`cargo test -p bc-engine-table`；额外压力：
`cargo test --release -p bc-engine-table -- --ignored`（1 GiB 流式记录、100 MB workbook 惰性/取消）。

接口与边界见 [ADR 201](../../docs/decisions/201-table-source-and-alignment.md)。
