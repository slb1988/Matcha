# table-compare-xlsx — Wave 7 Search 子项（4.4 查找/替换）回归与修复

日期：2026-11-06。执行：pi `k3.1-pro`。范围：ownership 确认后独占 `find.rs` 与 `commands.rs::find_action`（总控 delivery_27e22a537d3b 授权）；新增 `crates/bc-gui/src/table/search_tests.rs`（tests.rs 注册由主 worker 添加，未动）。其余源码归主 worker；未 commit/push。

## 交付与结果

- `search_tests.rs`：12 项真实行为回归（egui RawInput/真实 UI），全部通过；日志 `target/table-wave7/search/search-final.log`（另见 `search-6.log`）。
- 修复后 `table::tests::` 全套 79/80 通过；唯一失败 `sheets::same_file_two_sheets_via_manual_pairing_and_left_source_reordering`（`sheet_tests.rs:221`）是主 worker 正在迭代的文件，与本范围无关。
- 我的文件 clippy 零警告（全套唯一 warning 在 `sheet_tests.rs:71`，主 worker 范围）。

## 先复现后修复的三项（均在 ownership 段内）

1. **换侧后沿用旧命中坐标**：`find.side` 切换不重置 `last`，Next 从旧侧坐标之后继续，漏掉新侧前面的命中。修复：`FindPanel` 增加 `last_side`，`find_action` 仅在 `last_side == 当前侧` 时沿用位置；面板 Left/Right 单选 `.changed()` 时重置（`find.rs`）。复现测试 `side_selection_limits_hits_and_switching_side_resets_position`（修复前 cursor 落在行 3 而非行 0）。
2. **空替换/同值替换也建历史**：Replace 无当前命中、Replace All 替换为原值时都 push 空 `TableEdit`，文档变脏。修复：逐格跳过 `text == 原值`，`replacements` 为空时直接返回不建历史，消息 `Replaced 0 cell(s)`。测试 `replace_without_hit_or_same_value_creates_no_history`。
3. **替换后撤销路由错位**：Replace/All 把编辑写入 `history[s]` 但不切 `grid.active`，另一侧的 Ctrl+Z 无法到达刚发生的替换。修复：替换分支设置 `grid.active = s` 并 `focus_grid`（与 `copy()` 一致），由 `replace_all_is_atomic_and_undo_restores_everything` 钉住。

## 覆盖矩阵（对照验收条款）

| 条款 | 测试 |
|---|---|
| 打开（Ctrl+F / Search 菜单）与字面 Next 定位+消息 | `ctrl_f_and_search_menu_open_panel_literal_next_locates_cell` |
| 正则 / 大小写 / 全词 | `regex_case_and_whole_word_matching`（含“apple pie 中的 apple 是合法全词命中”语义钉住） |
| Next/Previous 方向与边界提示 | `next_previous_direction_sequence_and_boundaries` |
| Left/Right 侧限定 + 换侧重置 | `side_selection_limits_hits_and_switching_side_resets_position` |
| 换 query（真实键入查询框）重置 | `typing_new_query_through_ui_resets_position` |
| 零宽正则 / Unicode 不卡死 | `zero_width_and_unicode_patterns_do_not_hang` |
| 隐藏列/过滤行命中后显露 | `find_reveals_hidden_columns_and_filtered_rows` |
| Replace 一次 + Undo/Redo | `replace_once_then_undo_redo` |
| Replace All 原子（一次 Undo 全恢复） | `replace_all_is_atomic_and_undo_restores_everything` |
| 只读拒绝 Replace/All 且不建历史 | `readonly_side_rejects_replace_and_all` |
| 无命中/同值不建历史 | `replace_without_hit_or_same_value_creates_no_history` |
| 未就绪拒绝查找 | `find_requires_ready_comparison` |

## 明确不在本轮（按任务边界，不偷偷后置为门禁）

- 不新增“Replace 自动跳到下一命中”；当前替换需先 Next 定位，语义已由测试钉住。
- 同步全表扫描不本轮后台化；大表 Find/Replace All 的 UI 线程阻塞为**已记录未测风险**，不由本测试宣称。
- 测试期两个自愈项：全词语义数据设计（`apple pie`→`applepie`）、派生列默认 `use_default=true` 导致逐列 `unimportant` 须同时关 Use Default 才生效（与既有测试口径一致，非引擎 bug）。
