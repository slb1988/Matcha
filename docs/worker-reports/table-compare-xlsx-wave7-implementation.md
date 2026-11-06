# table-compare-xlsx — Wave 7：任务 4.4 会话/文件/多 sheet 交互实现与验收

日期：2026-11-06。基于 master HEAD `e3784b2`（21/23，仅剩 4.4/6.1）。本报告按阶段累计书写；**最终计数以文末最新一轮为准：表格 GUI 111 项、workspace 372 passed/0 failed/3 ignored**（前文 97/358 等数字为中间阶段快照，保留不改）。4.4 已由总控功能 review 通过并在 tasks.md 勾选（22/23）；6.1 仍未勾选，待同轮 A/B 基准与最终集成验证。

本轮模型：pi `k3.1-pro`（provider `pl-anthropic`）。与两名只读/对抗 worker 并行：审计 worker 的 `crates/bc-engine-table/tests/session_history.rs`（6 项）与 Search worker 的 `find.rs`/`find_action`/`search_tests.rs`（12 项）均为各自交付，本轮未改动其内容与断言。

## 结果概览

- 表格 GUI 测试：**53 → 97 项全部通过**（新增：session 25、sheet 7、search 12（归属 worker2）；53+44=97 已经 `cargo test -p bc-gui --lib table:: -- --list` 核实）。
- 引擎表格套件：`swap_orientation.rs` 新增 5 项 + `session_history.rs` 6 项（对抗）全过；bc-engine-table 14 个测试目标全绿。
- `cargo clippy --workspace --all-targets -- -D warnings`：零警告。
- `cargo test --workspace`：**358 passed / 0 failed / 3 ignored**（3 个忽略项为既有压力/手动测试，本轮未单独跑）。
- 定向 rustfmt：本轮自改文件全部 `rustfmt --check` 干净（`search_tests.rs` 的 1 处格式差异属 worker2 文件，未越权格式化；`session_history.rs` 属审计 worker）。
- 日志与证据：`target/table-wave7/implementation/`（final-*.log、pre-fix-*.log、evidence-session-swap-only.rs、pre-fix-swap-e3784b2.log）。

## 4.4 覆盖矩阵

| 4.4 行为 | 实现/修复 | 证据（测试） |
|---|---|---|
| 查找/替换（字面/正则、大小写/全词/方向/侧） | **Search worker 交付**（find.rs 侧切换重置 last、find_action 无命中/同值不建历史等） | `search_tests.rs` 12 项；另补 `session_tests::search_panel_buttons_and_keyboard_undo_flow_through_raw_input`（Ctrl+F→Next→Replace all→Ctrl+Z 真实输入流） |
| 会话设置五页 | Specs/Format/Columns/Alignment/Sheets 内存草稿沿用；**修复**：非法映射先验证后应用、不再污染 `default_rules`；Format 改动经 staged reload 真正重解析（列数变化可见）；重载失败保留旧会话与全部设置 | `settings_invalid_mapping_is_rejected_without_polluting_rules_or_defaults`（旧代码失败）、`settings_reload_failure_keeps_paths_options_and_the_running_comparison`、`format_settings_change_actually_reparses_the_file`、`closing_settings_without_apply_changes_nothing`（取消）、`settings_apply_through_the_real_window_takes_effect_and_accept_structure_clears_annotations`（真实窗口点击：Columns tab→Accept structure→Use Default→Unimportant→Apply） |
| 键推断标注 | 状态区与 Columns 页显示解析后的键列名+来源：`keys: id（自动）`/`（手动）` | `inferred_keys_are_annotated_with_column_names_in_status_and_settings`（旧代码失败，证据 pre-fix-keys.log） |
| 列结构接受/改写 | Accept structure 清除注释；改写经 DraftValue/行选择生效（见上 Apply 测试）；摘要/统计同步 | 同上；`structural_columns` 2→0 断言 |
| Table Compare Info | 菜单 Session→Table Compare Info 打开；每 sheet 行列五计数 + 当前 sheet 明细 + 重复键 | `table_compare_info_lists_per_sheet_counts_through_the_menu`（真实菜单点击） |
| Swap Sides | **引擎重设计**：`TableMap::swap_sides` 转置 sheets/rules(含列方向注释 Inserted↔Deleted)/anchors/state（镜像后 map 保持 aligned，不重比）；`TableEdit` 捕获方向奇偶位，Undo/Redo 跨 Swap 先转置快照再恢复；GUI 侧转置 `default_rules`、镜像 summaries 左右孤儿计数、不再整表重算 | `swap_orientation.rs` 5 项（引擎）、`session_tests::swap_*` 3 项（GUI，e3784b2 快照上 2 项失败，证据 pre-fix-swap-e3784b2.log）、`swap_mirrors_orphan_counts_and_statistics`、`swap_mirrors_cached_sheet_summaries_left_right_counts`（N4，pre-fix-summaries.log）；Swap 前 stale 快照被拒（session_history 第 3 测） |
| Reload | **staged**：成功才替换 pair/names/选项；失败保留旧比较并报错 | `reload_failure_keeps_the_previous_comparison_and_reload_success_rereads`（旧代码失败后 15s 永久 wedge，pre-fix-staged.log） |
| Recompare | 保留历史、后台重算 | `recompare_keeps_history_and_recomputes` |
| Open File 单侧 | **staged + 外科替换**：新文件按掩码重路由格式选项；加载成功才提交 files/options/pairs；失败保留整个会话；对侧（含**缓存 sheet**）编辑/历史按 sheet 源索引重新挂接保留；孤儿 tab 发起时不再用空表冒认真实 sheet | `open_file_replaces_only_that_side_and_a_failed_open_keeps_the_whole_session`、`open_file_routes_the_new_source_through_the_format_registry`、`sheet_tests::open_file_preserves_the_other_sides_cached_sheet_edits_across_repairing`（多 sheet 关键回归） |
| Open Clipboard 单侧 | 同上外科路径（同步解析）；workbook 侧剪贴板强制分隔符探测（N7）；替换侧历史清空、对侧保留 | `open_clipboard_replaces_only_that_side_and_preserves_the_other_history`、`clipboard_replace_preserves_cached_sheets_of_the_other_side_without_confirmation`、`sheet_tests::clipboard_on_a_workbook_side_detects_the_actual_delimiter`（pre-fix-n7.log） |
| 两侧独立 Save / Save As | 每侧独立 target/dirty；Ctrl+S 存活动侧；保存失败重排程 realign（不再永久 stale）；保存不再取消 sheet 扫描 | `save_as_updates_only_that_side_target_and_marks_it_clean`、`save_failure_rearms_realign_and_does_not_cancel_the_sheet_scan`（pre-fix-save.log）；既有 `structures_saved_fields_and_independent_history` 持续通过 |
| xlsx 只读提示 | workbook 侧 Save 报 read-only，CSV 导出可用 | `workbook_side_reports_readonly_on_save_and_export_still_works` |
| Sheets 手动配对/切换/色点 | Sheets 页 Move 跨配对持久生效（`fixed_pairs`，不再被按名推导覆盖）；select_sheet 同理（修复配对后切错 sheet 的缺陷，pre-fix-fixedpairs.log）；标签色点/pending 语义；扫描被取消后自动恢复（N2） | `same_file_two_sheets_via_manual_pairing_and_left_source_reordering`、`sheet_tabs_show_state_dots_and_switching_preserves_cached_edits`、`cancelled_sheet_scan_is_restarted_so_tabs_do_not_stay_pending` |
| 左文件为源 | 配对顺序随开关翻转（异序 sheet 夹具验证） | `left_file_is_source_decides_the_pairing_order` |
| 同文件两 sheet | Specs 两侧同路径 + Sheets 移动一侧 → 交叉配对 | 同上 same_file 测试 |
| 差异摘要导出 | 文本/CSV 按键行 + 列结构；陈旧结果拒绝（引擎 `is_stale` 守卫）；与 Ignore Unimportant 一致 | `summary_export_writes_keyed_rows_and_stale_results_are_rejected`（真实路径框 OK 点击） |
| Swap 期间初始加载 | pair=None 时 Swap 安全重启加载（N1） | `swap_during_the_initial_load_restarts_safely` |
| 初始加载失败 | 明确错误 + Retry 入口，不再永久 spinner | `failed_initial_load_shows_error_and_retry_instead_of_a_forever_spinner` |
| 脏确认 | 目标侧脏/孤儿占位侧脏/草稿脏 → 确认；Save changed files 仅对整替换（Home/Exit/Reload/Settings/Text）阻塞于缓存脏 | `replacing_a_dirty_side_asks_confirmation_and_discard_keeps_the_other_side`、既有 `home_masks_and_unsaved_cancel_do_not_discard` |

## 修复清单（对应总控反例与审计发现）

**引擎（bc-engine-table，已报备）：**

1. `map.rs`：`TableMap` 增加方向奇偶位；`swap_sides()` 转置 sheets/settings(rules.columns+overrides)/state（`TableAlignOutput::transpose` 镜像 rows/revisions/rules/overrides/duplicate_keys，CompactRow 左右互换）；`TableEdit` 捕获奇偶位，`restore` 在跨 Swap 时先转置 before/after 快照再恢复 —— Undo/Redo 永不把交换前朝向写回已交换的 sheet（总控反例 3）。
2. `map.rs::replace_sheet(side, sheet)`：原位替换单侧（对侧 SharedSheet/history 快照保持有效）；Automatic 模式下表头不一致或映射越界 → 回退自动映射+键推断（F2/N3/同列数重排）；清空 anchors 并 bump `anchors_generation`（F3）；`TableEdit::restore` 跨替换边界只恢复内容，不复活旧 anchors、不覆盖当前会话规则（review R6）。
3. `map.rs::set_alignment`：候选规则先本地解析、全部校验通过再一次发布；空 columns 的未解析自动请求采纳 worker 结果；拒绝路径不再先污染 settings（review P1，新增 `stale_output_is_rejected_without_touching_unresolved_settings`）。
4. `columns.rs::ColumnMapping::transpose`：Inserted↔Deleted 方向翻转（F1，摘要方向与全新比较一致）；`rules.rs::TableRules::transpose`；`rules.rs::validate` 恢复 `default_handling.validate()`（F4，系本轮编辑失误被总控增量 review 抓获）。
5. `Sheet` 无改动；bc-core/bc-engine-text/bc-session/prefs.rs 零修改。

**GUI（crates/bc-gui/src/table/）：**

6. `view.rs`：`Staged` 机制（Reload/Open/Settings/PairSide）——worker 成功才提交，失败保留旧会话；`install` 拆分为 `install`/`install_aligned`/`finish_install`；`maybe_spawn_scan` 统一扫描恢复（N2）；初始失败显示错误+Retry（review）；`swap()` 用 `swap_sides`+`project`；`select_sheet` 传 `fixed_pairs`（手动配对后切 tab 不再错配）并支持 pending_side 对的按需加载。
7. `commands.rs`：`perform(Open)` 按 registry 重路由新来源格式选项、孤儿 tab 不冒认；`commit_open_side` 按未替换侧 sheet 索引重新挂接 active+cached 对（review 收口）；`Pending::Clipboard` 同样重新挂接、保留 names[other] 全量；`request()` 对单侧替换按目标侧+孤儿占位侧+草稿判定确认（缓存对因保留不再需要确认）；`save_to` 拆出 `write_side`：只取消比较 worker 不取消 scan、失败重排程 realign；确认对话框的缓存阻塞只对整替换生效。
8. `worker.rs`：`LoadJob` 支持 `memory`（保留侧内存 sheet）、`fixed_pairs`（手动配对）、`active` 选择；加载前校验映射，不兼容自愈为自动（与引擎 replace_sheet 同契约）；新增 `pair_for_replacement` 共享配对helper。
9. `settings_panel.rs`：Columns 页键标注 + 字段 pub(crate) 供测试；`view.rs::keys_description`。

**Search（worker2 交付，未被我修改）**：find.rs 侧切换重置 last、find_action 无命中/同值不建历史等，12 项回归全绿。

## 新测试对旧代码的失败证据

| 证据 | 内容 |
|---|---|
| `pre-fix-swap-e3784b2.log` | 当前 3 项 Swap GUI 测试在 e3784b2 原实现（git archive 独立快照 + 独立 target）上 2 项失败：Undo 恢复交换前朝向规则、default_rules 未转置；fixture 前置断言在旧代码通过（排除 fixture 错误） |
| `pre-fix-staged.log` | staged 五测在旧代码失败：Open/Clipboard 丢对侧历史、Reload/Settings 失败后 pair=None 永久 wedge（15s 超时 panic） |
| `pre-fix-save.log` | 保存失败不再重排程 + 取消 scan |
| `pre-fix-keys.log` | 状态区无键列名标注 |
| `pre-fix-n7.log` | workbook 侧逗号配置读 TSV 剪贴板成一列 |
| `pre-fix-fixedpairs.log` | 手动配对后 select_sheet 回退第 0 对 |
| `pre-fix-summaries.log` | Swap 后非活动 sheet summaries 左右孤儿未对调 |
| 审计 worker | `session_history.rs` 6 测 4 败（F1-F4）→ 修复后 6/6（其日志在 target/table-wave7/review/） |

## 测试命令与结果

```
cargo test -p bc-gui table::            → 97 passed / 0 failed（final-gui-table.log）
cargo test -p bc-engine-table           → 14 个目标全绿（final-engine.log）
cargo test --workspace                  → 358 passed / 0 failed / 3 ignored（final-workspace.log）
cargo clippy --workspace --all-targets -- -D warnings → 0 警告（final-clippy.log）
rustfmt --check（本轮自改 12 个文件）    → 干净
```

## 未解决事项与边界（不冒称）

- **Search 验收口径**：worker2 的 12 项多为控制器级 `find_action` 调用；真实 RawInput 入口由我补的 1 项（Ctrl+F/Next/Replace all/Ctrl+Z）。按总控要求如实记录为"控制器回归 + 若干 RawInput 入口"。
- **大表 Find/Save/Export 仍为前台扫描/写盘**；后台化不是 4.4 门槛，未测大数据量耗时。
- **Swap 后不重比**依赖镜像转置的正确性（引擎 5 项 + 对抗 6 项覆盖 undo/redo/结构编辑/双 Swap/stale 拒绝）；多 sheet 缓存对在 Swap 后标记 stale，访问时重比。
- 单侧替换后，未访问的保留 sheet 标签显示 pending（色点待访问时实算），不用磁盘内容冒充内存编辑状态。
- Open File 重路由格式选项意味着：会话内对旧文件手工调整的 Format 选项不会带入新打开的文件（按新来源掩码匹配）；如总控认为应保留会话覆盖，需另立决策。
- 剪贴板替换在同名匹配时优先按名配对，否则与当前查看 sheet 配对（`pair_for_replacement` 强制对）；与参考工具"剪贴板对当前 sheet"一致，跨 sheet 按名扩展为本实现增强。
- 物理鼠标/原生 IME/锁定桌面仍按 ADR 202 留人工点验；6.1 的全量回归与同轮文本基准对照未在本轮运行（不并行跑原生性能基准，等总控派发）。
- `search_tests.rs`（worker2）有 1 处 rustfmt 差异未动；`session_history.rs`（审计 worker）格式未动。

## 文件清单（本轮修改）

- `crates/bc-engine-table/src/{map.rs, rules.rs, columns.rs}`、`crates/bc-engine-table/tests/swap_orientation.rs`（新）
- `crates/bc-gui/src/table/{view.rs, commands.rs, worker.rs, settings_panel.rs, tests.rs}`、`session_tests.rs`（新）、`sheet_tests.rs`（新）
- 未修改：app.rs、home.rs、bc-core、bc-engine-text、bc-session、prefs.rs、folder 源码、openspec/specs、tasks.md、Cargo.lock；未新增依赖；未触碰 tmp/、vault、其他 worktree、参考程序。
- worker2 修改：`find.rs`、`commands.rs::find_action`、`search_tests.rs`（其报告见 wave7-search.md）；审计 worker：`session_history.rs`。

## 最终 review 回归修复（R1/R2，2026-11-06 追加）

总控 4.4 最终 review 的两个 blocking 项，均先写反例再修（`crates/bc-gui/src/table/review_tests.rs` 3 项；旧代码失败证据 `target/table-wave7/review-final/pre-fix-r1r2.log`）：

- **R1**：`request()` 对 Open/Clipboard 重新纳入 `cached.values().any(|p| p.history[s].is_dirty())`（仅**被替换侧**的缓存脏编辑触发确认；未替换侧缓存编辑因重新挂接保留而不需确认）。`replacing_a_side_with_dirty_cached_target_edits_requires_confirmation` 以真实 request→对话框 Cancel/Discard 覆盖 Open 与 Clipboard 两路径：Cancel 全保留；Discard 仅目标侧内容/历史被替换，对侧缓存编辑与 Undo 经实际访问 sheet 验证仍在。
- **R2**：`maybe_spawn_scan` 启动条件改为"存在非活动且非缓存的待扫描 pair"（扫描结果本就跳过 cached，cached pending 不再构成重启条件）；扫描失败置 `scan_failed` + 明确错误消息（"switch sheets or Reload to retry"），不忙重启、不把 pending 伪装成 Same；`cancel()`（用户动作）清除失败标记构成重试路径。`retained_pending_cached_sheets_do_not_retrigger_the_scan_forever`（替换后 5 帧 poll 不重启，旧代码立即重启）、`a_failing_sheet_scan_reports_error_and_waits_for_user_retry`（失败→消息→4 帧不重启→恢复文件后扫描完成）覆盖；既有 `cancelled_sheet_scan_is_restarted_so_tabs_do_not_stay_pending` 与多 sheet 套件持续通过。

修复后计数：表格 GUI **100 项全绿**（97+3），workspace **361 passed / 0 failed / 3 ignored**，clippy 零警告，三个改动文件 rustfmt 干净（final-gui.log / final-workspace.log）。6.1 未在本轮运行或宣称。

## 最终 review 第二轮收口（pending 生命周期/写保护/format_important，2026-11-06 再追加）

总控第二轮 review 的四个 blocking/收口项，先写反例再修（review_tests.rs 新增 5 项；旧代码失败证据见 `target/table-wave7/review-final/pre-fix-r1r2.log` 追加段）：

- **pending_side 生命周期**：`Staged::PairSide` 成功发布后清 flag（失败/取消保留以重试）；`swap()` 翻转 pending 侧并保持 sheet 索引；`project()` 不再把 stale/pending 缓存对的旧快照冒充新状态（保持 pending 点）；抽出 `load_pending_pair()` 供 select_sheet/Swap/Recompare/投影共用——占位表永不被比较或报告 ready。回归：`pending_side_clears_after_load_and_edits_survive_a_round_trip`（加载→编辑→切走→再回不重载覆盖）、`swap_flips_pending_side_and_the_visit_loads_the_correct_side`、`failed_pending_load_keeps_write_protection_and_recompare_restarts_the_load`。
- **写保护状态机**：`source_pending()`（staged 或活动对 pending_side）统一守卫 begin_edit/paste/clear_selected；`save_to` 在 staged/pending 时直接拒绝且**不取消加载**（先前后退会取消加载并把空占位写回原 xlsx）；`is_ready()` 在 staged/pending 时为 false；正常 deferred realign 窗口保持可编辑（不回归此前误挡）。回归：`save_and_writes_are_refused_while_the_target_side_is_pending`（原文件字节不变+加载继续+完成后导出可用）。
- **format_important 接 GUI**：Session Settings → Columns 页新增 "Format differences are important" 复选框绑定 `draft.rules.format_important`，Apply 经既有重比路径生效。回归 `format_importance_checkbox_in_settings_toggles_the_format_layer`：All 档 U → 勾选 Apply → I → Ignore 不隐藏 I → Values 档 Same → 取消路径不变。
- 修复过程中发现并改正一处自身测试缺陷（多测试共用同名临时 xlsx 导致并行竞态），已按测试加唯一标签。

本轮后计数：表格 GUI **105 项全绿**（100+5），workspace **366 passed / 0 failed / 3 ignored**，clippy 零警告，改动文件 rustfmt 干净（final 日志见 target/table-wave7/review-final/）。6.1 由总控另行派发，不在本轮宣称。

## 最终 review 第三轮收口（Save 决策/换源重配对/Swap 草稿/规则 Apply 草稿保护，2026-11-06 再三追加）

- **R1 Save 决策补齐**：单侧替换确认框点 "Save changed files" 时，若缓存 sheet 的**被替换侧**有脏编辑（workbook 无法写回），保持 pending 并提示导出/取消，不伪装保存成功；未替换侧缓存脏不阻塞单侧操作。回归：`save_decision_blocks_when_a_cached_target_side_is_dirty`（Save 点击后 pending 保留+内容/历史不丢+Cancel 全保留）。
- **Settings 换源重配对**：`perform(Settings)` 仅在来源路径不变时传 `fixed_pairs`/旧 active 位置；路径改变时按新文件 sheet 名重新配对并采用 `loaded.active`，修复旧索引越界返回空 sheet 冒充成功的回归。回归：`settings_with_changed_paths_repair_sheets_and_do_not_reuse_stale_indices`；既有同路径手动重排回归持续通过。
- **Swap 草稿转置**：`swap()` 将未提交 `CellEditor.side` 翻转（源坐标不变，镜像比较坐标不变，验证作用于同一物理 sheet），并使 `find.last` 失效（命中是侧局部的）。回归：`swap_transposes_an_uncommitted_draft_to_its_physical_side`（两侧同值，Swap 后 Commit/Undo 只作用原文件）。
- **rules-only Settings Apply 草稿保护**：`request()` 恢复"已修改编辑器草稿同样触发确认"，修复 rules-only Apply 静默丢草稿。回归：`rules_only_settings_apply_does_not_silently_drop_an_open_draft`（Cancel 留草稿/Discard 后应用生效）。

本轮后计数：表格 GUI **109 项全绿**（105+4），workspace **370 passed / 0 failed / 3 ignored**，clippy 零警告，改动文件 rustfmt 干净。四项均先录旧代码失败（pre-fix-r1r2.log 追加段）。

## 最终发布前两个精确漏修点（2026-11-06 追加，review_tests.rs +2 项）

- **Settings 换源 active 发布**：`Staged::Settings` 提交臂先写 `files[].path` 再比较 `draft.paths != self.paths()`，恒为 false 导致沿用旧 active 索引（2→3 个全新名 sheet 时 active=1 显示错配内容）。改为无条件采用 worker 解析的 `loaded.active`（同路径 fixed_pairs 本就有准确 active）。回归 `settings_with_changed_paths_publish_the_loaded_active_pair`：旧 active=1 换 3-sheet 新源后 active=0、标签为 ● Alpha、内容为实际 Alpha 数据（先失败后通过，pre-fix-active-scan.log）。
- **staged 事务期间不启动扫描**：`maybe_spawn_scan` 在 `staged.is_some()` 时直接返回——事务开始时 cancel 已丢旧扫描，事务中以旧 paths/pairs 启动的扫描没有 generation 守卫，可能按索引污染提交后的新结构；成功/失败恢复稳定状态后正常一次扫描。回归 `staged_transactions_do_not_spawn_scans_and_scan_resumes_afterwards`（staged 中两帧 poll 不 spawn；恢复后一次完成；R2 既有的失败不重试/取消恢复持续通过）。

本轮后计数：表格 GUI **111 项全绿**（109+2），workspace **372 passed / 0 failed / 3 ignored**，clippy 零警告，view.rs/review_tests.rs rustfmt 干净（final-gui-2.log / final-workspace-2.log）。

本 dispatch 收口时 Orca inbox 已 check+ack 至空，无未消费消息/未回答问题。
