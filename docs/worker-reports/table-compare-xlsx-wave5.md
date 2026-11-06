# table-compare-xlsx — Wave 5 确认排序方案实现

日期：2026-11-06。基线：`a0b8cd7`；按用户对排序方案的明确确认实施，在 master 直接工作，未派发 worker。

## 结果：排序子项完成，整体仍为部分交付

实现已确认规则：**按所选可见比较列顺序稳定多键排序；跨行选区只排序所选可见行，否则排序当前侧全表**。补入 `table-compare-view/spec.md` 的排序说明和两个场景、design D7 及命令对照 C42，不修改协调者的 `table-cell-comparison` / `table-columns-and-sheets` 两份语义 spec。

OpenSpec **仍为 18/23**。本轮没有新增勾选：4.3 的排序子项已完成，但其他交互/性能验收尚未闭合；4.1–4.4、6.1 保持未完成，不归档。

## 行为与实现

- 比较列到当前侧源列的映射决定排序键，优先级保持比较列的显示顺序，不按源列号重新排列。隐藏列及该侧不存在的列不充当排序键。
- 跨行与否由**原始 anchor/cursor 的比较行**决定，不由过滤后的行数决定。跨行选区被过滤成一行时不会误扩展为整表；无有效行/列、Show None、全为空隙时不排序。
- 跨行排序先解析实际源行集合，将排序结果回填原来的源行位置；隐藏和未选中的源行保持原位。源行重复索引去重，等键行在升序、降序中都保持原源顺序。
- 有效单行/仅列选区按确认规则排序整侧，**即使当前显示了 Differences 过滤**；菜单 tooltip 说明这个范围区别。
- `Sheet::sorted_row_order` 只规划置换，复用现有键排序的 Handling 规范化与值全序；General 数值排序、Text 词法排序、混合类型和忽略大小写保持一致。容差不用于判定排序相等，避免非传递比较关系。
- 一次 `TableMap::edit_sheet` 应用整次排序并建立一个历史项；原字段拼写、Cell 类型/公式/样式随源行移动，之后沿用后台重比较。只给实际移动的位置增加行编辑标记，不把未选中行标成新编辑。
- 排序不强制把自动键改为 Manual，不改变 `sort_by_key` 等对齐设置；若视图启用了排序后对齐，重比后的显示仍遵循该设置，而源文件行序按 Sort 的结果保存。
- 实际重排沿用原结构编辑的清除手动锚点策略，Undo 可还原；顺序未变则不清锚点、不改变 revision、不创建历史或破坏 Redo。
- 只读、未提交草稿或陈旧比较期间拒绝排序，不回退到猜测的源行/列。菜单操作后恢复活动网格焦点，键盘 Undo/Redo 有回归。

## 回归证据

最先加入的 **5 项 GUI 行为回归在旧实现全部失败**：单列代替多键、跨行过滤误排整表、排序视图源行映射错误、缩水选区扩大范围以及无变化产生历史。修复后通过，并扩展到以下 **14 项新增测试**：

| 文件 | 数量 | 覆盖 |
|---|---:|---|
| `crates/bc-engine-table/tests/sorting.rs` | 6 | 稳定多键/非连续源行位置、General/Text、混合类型全序、规范化等键/容差不影响顺序、非法/空/恒等计划、整 Cell 与词法字段搬移及未选行编辑标记 |
| `crates/bc-gui/src/table/sort_tests.rs` | 8 | RawInput 菜单升降序、错序列映射、键盘 Undo/Redo、过滤跨行/整侧范围区分、排序后对齐的源索引、自动键保持、隐藏列、空/空隙/只读/草稿/陈旧保护、锚点与无变化 Redo 保留 |

真实输入证据为 egui RawInput，不冒称物理鼠标或原生 IME 已完整人工验收。

## 验证结果

| 命令/检查 | 本轮结果 |
|---|---|
| `cargo clippy --workspace --all-targets -- -D warnings` | 退出 0，零 warning |
| `cargo test --workspace` | **284 passed / 0 failed / 3 ignored** |
| `cargo build --release --workspace` | 退出 0 |
| `cargo test -p bc-engine-table --test sorting -- --nocapture` | 6 passed |
| `cargo test -p bc-gui table::tests::sorting -- --nocapture` | 8 passed |
| `cargo test -p bc-engine-table --test golden_table -- --nocapture` | 3 passed；30 例仍为行 81/83（97.59%）、格 236/242（97.52%），仅既有 `024-unaligned` 差异 |
| `python -B -m unittest discover -s tools/golden -p test_table_parity.py` | 3 passed；本轮没有重新解包/扫描参考帮助 |
| `openspec validate table-compare-xlsx --strict --json` | valid=true，0 issues |
| 本轮 8 个 Rust 文件定向 rustfmt、`git diff --check` | 通过；未全仓格式化 |

本轮未重跑默认忽略的两项表格压力测试、手动回收站测试、GPU 滚动基准或百万行 Sort 基准。Wave 4 的 p99 不能作为本轮新测量。排序键预计算、全行置换及重建目前在命令路径执行，尚无百万行排序耗时/峰值内存/取消性的验收结论；不能据此勾选 4.3 或 6.1。

日志位于忽略的 `target/table-wave5/`：`sort-before.log`（旧实现失败证据）、`sort-engine.log`、`sort-gui.log`、`clippy.log`、`test.log`、`release.log`、`golden.log`、`openspec.log`、`parity.log`、`fmt.log`。

## 剩余与提交

- 4.3：排序范围决策已解决；快速连续结构命令、Align With/清除的完整输入矩阵、完整 IME/面板焦点及大规模编辑性能仍待验。
- 4.1/4.2/4.4：沿用 Wave 4 的显示/导航、自定义格式、Swap/多 sheet/设置/文件操作等剩余矩阵。
- 6.1：完整交互与同轮性能对照尚未收敛。

| 范围 | commit |
|---|---|
| 排序实现、源行编辑标记及 14 项回归 | `ee07dc0cabb09841ac8e5186a2f883285e491680` |

确认后的 spec/design、任务跟踪、对照及本报告另作 docs 提交，提交号可用 `git log -1 --format=%H -- docs/worker-reports/table-compare-xlsx-wave5.md` 查询。

无新增依赖、无 Cargo.lock/App/Home/文件夹源码修改；未访问其他 worktree 或 vault，未修改 tmp 输入、调用参考工具或提交参考图标/私人配置/二进制 fixture。未 push，未归档。
