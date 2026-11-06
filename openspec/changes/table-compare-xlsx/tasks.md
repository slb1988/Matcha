# Tasks

说明：在 master 上实现，与 `folder-compare-sync` 分支并行。全程约束：`cargo clippy --workspace --all-targets -- -D warnings` 零警告、`cargo test --workspace` 通过、新增第三方依赖仅 `calamine`、引擎 crate 不依赖 GUI、`bc-core`/`bc-engine-text`/`bc-session` 零修改、不改动 `tmp/` 与 vault、调用参考工具前持 `%TEMP%\matcha-reference-tool.lock`。每个任务组至少一个 Conventional Commits 提交。

## 1. 模型与来源

- [x] 1.1 实现列式 `TableModel`/`Sheet`/`Cell`（类型化值、公式、样式 id 去重、合并范围）与 `bc-formats::TableOptions` 的 Type（Delimited/Fixed、分隔符集合、限定符、连续分隔符、环绕空白、自动检测、列宽）与 Regional 字段扩展；验证单测：模型构造与 `TableOptions` XML 读→写→读等价，内置 CSV/MSEXCEL 仍解析
- [x] 1.2 实现分隔文本解析器（RFC 4180 引号与换行、连续分隔符、环绕空白、自动检测）与固定列宽解析；验证单测覆盖 table-source-formats 的分隔/固定场景与 1 GB 级流式读取不整文件入内存
- [x] 1.3 实现表头检测与覆盖、区域设置解析（小数/千分位/日期顺序与分隔符）、编码与 Ctrl+Z EOF；验证单测：自动表头、强制数据、欧式数字、日期解析
- [x] 1.4 接入 `calamine`：枚举 sheet、读取值/类型/公式/合并范围、按 sheet 惰性加载与取消、日期序列值解释；验证单测用代码生成的最小 xlsx（zip + sheet xml，不入库二进制）覆盖多 sheet、公式、日期、布尔、错误
- [x] 1.5 实现分隔文本保存（保留分隔符/限定符/行尾，必要时加限定符）与 xlsx 只读保护；验证单测：改一格保存后仅该字段变化

## 2. 列、键与 sheet

- [x] 2.1 实现比较列映射（左右列号、Move Up/Down、Tidy、按列号默认）、默认与逐列 `ColumnHandling`（Key、Use Default、Type、Unimportant、大小写/空白、日期与数值容差）；验证单测：映射与 Handling 的往返与生效
- [x] 2.2 实现键列自动推断评分（D3）与"自动/手动/全部列/按列号"状态；验证单测：典型 id 列、复合键、无合适键三种语料
- [x] 2.3 实现列对齐与结构变化识别（表头名匹配、内容相似度、插入/删除/重命名/移动标注）；验证单测：中间插入列、重命名、移动、列数不同
- [x] 2.4 实现 sheet 配对（名称归一化、手动重排、左文件为源、同文件两 sheet）；验证单测：同名配对、手动配对、单侧 sheet

## 3. 行对齐与单元格比较

- [x] 3.1 实现键哈希与 `bc_core::align` 的接入（Quick/Smart/Patience/None、closeness、`AlignmentOverrides`）、重复键处理、排序后对齐；验证单测：table-row-alignment 全部场景
- [x] 3.2 实现 `CellComparator`（类型/容差/大小写/空白规范化）与值/公式/格式三层、紧凑行状态位图、行与 sheet 汇总、Ignore Unimportant 投影；验证单测：table-cell-comparison 全部场景
- [x] 3.3 实现 `TableMap`（派生行、导航：差异单元格/行/编辑、统计、`copy_cell/copy_row` 作为 `EditItem`、键改变后重对齐）与 `TableAlignJob`/`TableRealignJob`（快照 + 修订号）；验证单测：导航顺序、复制后相同且可撤销、过期结果拒绝
- [x] 3.4 实现差异摘要导出（文本/CSV：按键的行状态与变化列旧值→新值、列结构变化、遵循 Ignore Unimportant）；验证单测：规格示例 `7 | price | 1.0 -> 1.2`
- [x] 3.5 基准 `examples/table_bench.rs`：生成两份 100 万行 × 10 列 CSV（临时目录），测读取、键推断、对齐、比较耗时与内存；验证对齐在后台可取消，数字写入报告

## 4. GUI

- [x] 4.1 实现 `bc-gui/src/table/` 双网格（只渲染可见行列、同步滚动、行状态色点、表头、缩略图、Text Details、File Info、Side-by-Side/Over-Under、行号、可见空白、Resize Columns、Hide Same Columns），`app.rs` 加 `View::TableCompare`、`home.rs` 加表格卡片与按掩码路由；验证手动 + egui 输入注入回归：百万行网格滚动 update p99 < 4 ms（`--bench-table`）
- [x] 4.2 实现导航与显示过滤（Next/Previous Difference Cell/Row、Next/Previous Edit、Go To、Show All/Differences/Same/None、Ignore Unimportant）；验证输入注入回归对照 table-compare-view 场景
- [x] 4.3 实现编辑（Modify、剪贴板、插入/删除行列、Sort、Copy to Other Side、Copy Row、Select All、Align With、撤销重做）与键改变重对齐；验证输入注入回归：复制单元格到对侧后相同、修改键后重新配对
- [ ] 4.4 实现查找/替换、Go To、会话设置面板（Specs/Format/Columns/Alignment/Sheets，含键推断标注与列结构变化接受/改写）、Table Compare Info、Swap/Reload/Recompare、两侧独立保存与 xlsx 只读提示、sheet 标签色点与切换、差异摘要导出入口；验证输入注入回归对照规格
- [x] 4.5 三层档位切换（只看值 / 值+公式 / 值+公式+格式）与 Text Details 显示两侧公式；验证回归：值同公式不同用例

## 5. 黄金语料与对照

- [x] 5.1 编写 `tools/golden/gen_table_expected.ps1`：≥ 20 个 CSV 用例（键对齐、排序、重复键、容差、不重要列、大小写/空白、列数不同、多 sheet xlsx 由参考工具转换）用参考工具 `table-report layout:side-by-side output-options:html-color` 生成期望并入库，持互斥锁与备份/恢复纪律；验证脚本本机跑通
- [x] 5.2 编写 HTML 期望解析器与回归 `bc-engine-table/tests/golden_table.rs`（语料为空时跳过）；验证行状态与差异单元格一致率 ≥ 95%，不一致写 `tests/golden/table/KNOWN_DIFFS.md`
- [x] 5.3 编写 `docs/reference-parity/table-compare.md`（D9 逐条命令/选项）；验证覆盖 `commandstable.html` 每条命令

## 6. 集成

- [ ] 6.1 全量回归（clippy `-D warnings`、`cargo test --workspace`、黄金一致率、`--bench-table`、文本线 `--bench` 不退化）并写报告 `docs/worker-reports/table-compare-xlsx-*.md`

## Workflow follow-up

- 评审通过后归档本 change；xlsx 写回与格式层扩展另立 change。
- Wave 2 历史收口为**部分交付**，当时仅确认 5.2 完成；证据见 `docs/worker-reports/table-compare-xlsx-wave2.md`。Wave 3 补齐 5.1、5.3，见 `docs/worker-reports/table-compare-xlsx-wave3.md`。
- Wave 4 补齐 4.5 的三档实际输入/公式详情/数字格式回归，并修复批量复制与显示层切换的扫描取消问题；累计 18/23。当时多列排序的范围决策待确认，证据与限制见 `docs/worker-reports/table-compare-xlsx-wave4.md`。
- Wave 5 按用户确认实现稳定多键排序及跨行选区范围，补升降序、映射、过滤和撤销/重做回归；详见 `docs/worker-reports/table-compare-xlsx-wave5.md`。4.3 其他交互/性能验收仍有缺口，故累计仍为 18/23，4.1–4.4、6.1 不勾选。构建成功不等同于全部交互验收完成，暂不归档。
