# table-compare-xlsx — Wave 7 引擎 Swap/replace_sheet 对抗回归（独立 review）

日期：2026-11-06。审查人：pi `k3.1-pro`。对象：工作区未提交的 `bc-engine-table` diff（`map.rs` 新增 `swap_sides`/`replace_sheet`/orientation parity，`columns.rs`/`rules.rs` 新增 `transpose`；GUI `commands.rs` 改调 `map.swap_sides()`）。源码修复归主实施 worker，本文件只新增 `crates/bc-engine-table/tests/session_history.rs`（公开 API、clippy 零警告），不改任何已有源码/测试/文档。

## 结果总览：2 通过 / 4 失败（失败=可复现审查发现）

日志：`target/table-wave7/review/session_history-final.log`；同 crate 其余套件（含主 worker 的 `swap_orientation.rs`）全部通过，见 `engine-suite.log`。

| 测试 | 结果 | 说明 |
|---|---|---|
| `swap_while_stale_rejects_preswap_snapshot_and_realigns` | ✅ | deferred stale 中 Swap：stale 保持；Swap 前完成的重对齐结果被 `set_alignment` 拒绝不安装；当前方向重对齐+Undo 后统计==全新比较 |
| `structural_edit_undo_redo_across_swaps_keeps_rules_anchors_statistics` | ✅ | 非对称映射+非对称锚点下，插入列跨两次 Swap 的 Undo/Redo：源值、规则方向（转置正确）、锚点 `(0,1)↔(1,0)`、统计与 ground truth 全等 |
| `swap_flips_directional_column_annotations_in_summary` | ❌ F1 | `ColumnMapping::transpose` 只交换 left/right，不翻转 `changes` 的方向性注释：Swap 后 `export_summary` 该列仍输出 `Deleted`，而对当前两侧全新比较为 `Inserted`（按同一比较列逐项核对方向与旧值→新值、行记录按集合核对、不强求字节序——总控口径；diff：`["COLUMN\t3\tDeleted\t-\t3\t\tb"]` vs `["COLUMN\t3\tInserted\t-\t3\t\tb"]`）。用户可见：Swap 后差异摘要的列结构方向错误 |
| `replace_sheet_with_fewer_columns_must_stay_recomparable` | ❌ F2 | 换成列数更少的一侧后，`TableRealignJob` 直接 `Err("duplicate or out-of-range mapped column")`：旧列映射越界、无自动恢复，不撤销任何编辑也永久 stale。这是 4.4 单侧 Open File/Clipboard（K2）接线 `replace_sheet` 的阻塞点——调用方契约（清空被替换侧历史）不足以让比较继续 |
| `replace_sheet_then_other_side_undo_must_not_restore_stale_anchors` | ❌ F3 | 对侧 Undo 经 `TableEdit::restore` 无条件写回捕获的 `settings.overrides`：锚点 `[(1,0)]` 指向被替换后新 sheet 不存在的行（新左侧仅 1 行），且后续重对齐将报 `left anchor out of range` |
| `validate_rejects_invalid_default_handling` | ❌ F4 | 本轮 diff 删除了 `rules.rs::validate` 的 `default_handling.validate()?`：`numeric_tolerance=-1.0` 的默认 Handling 现可通过校验并扩散到全部 Use Default 列。若非有意删除，请恢复 |

## 建议修复方向（供主 worker 参考，不代为实施）

- F1：`transpose` 时交换 `Inserted↔Deleted`（Renamed/Moved 方向无关）；或 Swap 后对非空 `changes` 的映射重跑结构标注。
- F2/F3：`replace_sheet` 时清除引用被替换侧的锚点，并对越界列映射给出恢复路径（被替换侧列索引置空重推导或拒绝并提示）；`TableEdit::restore` 写回 settings 前需感知“捕获后发生了替换”（如代数/修订戳）。
- F4：恢复 `default_handling.validate()?` 或说明删除理由并补等价校验。

## 边界

未 commit/push；未修改任何源码；未跑 GUI/基准。测试断言均为用户可见规格（摘要方向、源值、规则/锚点、统计与全新比较一致、校验拒绝非法输入），未发明语义。F1–F4 已发总控（msg_2f020050bec6）。
