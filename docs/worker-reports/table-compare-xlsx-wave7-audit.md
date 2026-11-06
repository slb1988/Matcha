# table-compare-xlsx — Wave 7 只读审计（4.4 全覆盖缺口 + 6.1 同轮 A/B 准备）

日期：2026-11-06。审计人：pi `k3.1-pro` / `pl-anthropic`（环境证据见 `target/table-wave7/audit/environment.txt`，不输出任何秘密）。
对象：master `e3784b2`。审计时点工作树与该提交 `git diff` 为空；此后实施 worker 已开始并发修改（如 `bc-gui/src/table/tests.rs`、`session_tests.rs`），**本报告所有源码 `path:line` 均为 e3784b2 快照口径，实施修改后以最新代码评审为准**。本轮**只读**源码/specs/ADR/跟踪文档，未跑 clippy/test/构建（避免与实施 worker 并发争用 `target/`）；引用的门禁数字均来自 Wave 6 报告，不冒充本轮复测结果。

## 审计范围与依据

- 任务：`openspec/changes/table-compare-xlsx/tasks.md` 4.4（未勾选）与 6.1（未勾选）。
- 规格：`specs/table-compare-view/spec.md`（查找替换/会话命令与保存/sheet 标签/摘要导出为大头），`specs/table-columns-and-sheets/spec.md`（sheet 配对），`specs/table-source-formats/spec.md`（保存语义）。
- 决策：`docs/decisions/201`（来源/对齐边界）、`docs/decisions/202`（deferred edits、后置项）。
- 对照：`docs/reference-parity/table-compare.md`（77 段命令逐条状态）。
- 源码：`crates/bc-gui/src/table/{view,commands,find,settings_panel,sheet_tabs,cache,worker,grid,details,selection,sorting,formats,regional}.rs`，`crates/bc-engine-table/src/{map,editing,model,rules,sheets,summary,source,jobs,columns}.rs`，`crates/bc-gui/src/app.rs`。
- 既有测试：`tests.rs` 6 + `navigation/command/edit/copy/sort/projection/view_tests.rs` 47，共 53 项 GUI 回归（Wave 6 口径）。

## 4.4 逐项验收矩阵

状态列：✅已有实现且有回归 / 🟡已有实现但矩阵有缺口 / ❌缺口。每项给出可复核的 `path:line`。

| # | 4.4 子项（规格出处） | 实现入口 | 既有回归 | 状态 | 缺口 / 待验 |
|---|---|---|---|---|---|
| 1 | Find / Find Next / Previous（字面/正则/大小写/全词/方向/指定侧） | `find.rs:15-80`；`commands.rs:756-828 find_action` | `tests.rs:194` 一次 Next | 🟡 | 全表逐格**同步**正则扫描（`commands.rs:772-801`），大表上阻塞 UI 的未测风险（见 N6）；Find Previous 仅扫一次无回归；F3 快捷键有（`commands.rs:930-941`） |
| 2 | Replace / Replace All | `commands.rs:829-862` | `tests.rs:194` 一次 All+Undo | 🟡 | **N5**：无当前命中时 Replace 静默 0 替换；替换后 `edited()` 清 `find.last`（`view.rs:450`）不自动前进，连续 Replace 第二次无效；正则零宽/方向组合无回归 |
| 3 | Go To（行、列） | `view.rs:639-662 go_to`（工具栏 Go） | `navigation_tests.rs` 边界/显露 | ✅ | 4.4 轮次建议补一条与 4.2 的联动断言即可 |
| 4 | 会话设置面板 Specs/Format/Columns/Alignment/Sheets | `settings_panel.rs:183-292`；应用 `commands.rs:1150-1210` | 无专项 | 🟡 | **K4**（已知）：`commands.rs:1153-1156` 先写 `default_rules` 后校验，失败仍污染；reload 分支（`commands.rs:1173-1189`）**完全不校验** draft.rules，非法值只能等后台 LoadJob 失败。Description 字段不落盘（面板内存，`settings_panel.rs:195-198`）；Alignment 页切 left_source 直接 `SheetPairs::by_name` 覆盖手动配对（`settings_panel.rs:270-272`）；Sheets/Columns 页共享 `move_side`，0 值在 Sheets 页被当 Right（`settings_panel.rs:286`） |
| 5 | 键推断标注 + 列结构变化接受/改写 | 状态栏 `view.rs:687`；Columns 页 `settings_panel.rs:225-256` | 引擎层有 | 🟡 | “Accept structure” 清 `changes`（`settings_panel.rs:245`）已有；“改写”（按当前文件重列比较列）无入口；自动键标注仅有 label 无评分值 |
| 6 | Table Compare Info | `commands.rs:1323-1344` | 无专项 | 🟡 | 未扫描 sheet 标 pending 属实（不伪装 Same）；**N4**：Swap 后缓存 sheet 的左/右孤儿计数保持交换前口径 |
| 7 | Swap Sides | `commands.rs:1211-1237 swap` | 无专项 | ❌ | **K3**（已知，已复核行号）：`TableEdit.side`/快照 revisions 不随 swap 反转（`map.rs:562-612` restore 按旧 side 写 `state.revisions.0/.1`），swap 后 Undo 旧编辑会把修订号写到错误一侧、rules 快照列方向未反转。**N1**：初始加载中 Swap 无守卫，`recompare()` 在 `pair=None` 时空转（`view.rs:471-482`），worker 已被 `cancel()` 丢弃 → 永久 spinner。**N2 同类**：swap 丢 scan 且不重启 |
| 8 | Reload / Recompare | `view.rs:267-283 reload`、`view.rs:471-482 recompare`；`commands.rs:1087-1099` | 脏确认有（`tests.rs:341`） | 🟡 | **K1**（已知）：初始/重载失败仅留 message（`view.rs:402-406`），`pair=None` 时中央区永久 spinner。Recompare 保留历史属实（`install` 仅换 alignment，`view.rs:284-336`） |
| 9 | 两侧独立 Save / Save As / xlsx 只读提示 / Export CSV | `commands.rs:977-1071 save_to`；菜单禁用+提示 `view.rs:739-764` | `tests.rs:257` 保存字段保留 | 🟡 | 原子写 temp+rename（`source.rs:744-771`）、备份（`commands.rs:1008-1017`）、workbook 拒写提示均有。**K5**（已知）：`save_to` 调 `cancel()`（`commands.rs:981`）丢 scan 且不重启。**N8**：摘要/CSV 导出 `fs::write` 直接覆盖无备份无确认（`commands.rs:1316`）；同步串行化在 UI 线程（N6） |
| 10 | sheet 标签色点与切换 | `sheet_tabs.rs:6-66`；`view.rs:535-575 select_sheet`；后台 `worker.rs:236-283 ScanJob` | 无专项 | ❌ | **N2**：`select_sheet` 先 `self.cancel()`（`view.rs:539`）丢 scan；缓存命中走 `project()`，只有 `Loaded` 分支（`view.rs:368-376`）重启 scan → 切回已访问 sheet 后其余标签永久 pending。**N4**：swap 后缓存 summaries 口径未反转 |
| 11 | 手动 sheet 配对 / 同文件两 sheet | `settings_panel.rs:274-291`；`sheets.rs:46-74 move_to` | 引擎层有 | 🟡 | Apply 后 `perform` 仅在路径变化时重置 names 并采用 by_name 配对（`commands.rs:1161-1165` + `view.rs:355-358`），手动配对在换文件时被静默重排（合理但须测试钉住）；GUI 矩阵缺 |
| 12 | 单侧 Open File / Open Clipboard 替换 | `commands.rs:1100-1138`；拖放 `view.rs:651-662` | 无专项 | ❌ | **K2**（已知）：`Pending::Open` 全量 `reload()` 丢对侧编辑/历史；`Pending::Clipboard` 经 `from_sheets` 保留对侧内容但历史清零且不再 dirty（静默丢撤销+丢保存提示）。**N3**：`from_sheets`（`view.rs:185-210`）复用旧 `default_rules.columns/keys` 未清空（对比 `reload` 在 `view.rs:273` 清空），列数不符→对齐失败卡加载；巧合相符→静默错配。**N7**：剪贴板文本按该侧 xlsx 默认逗号选项解析（`commands.rs:1110`+`formats.rs:33-51`），TSV 粘贴列错位 |
| 13 | 差异摘要导出入口 | Session 菜单 `view.rs:594-609`；`summary.rs:62-201` | 引擎层 3.4 有 | 🟡 | stale 拒导出（`summary.rs:64-66`）、遵循 Ignore Unimportant、列结构变化列出——均属实；GUI 入口无回归；N6/N8 见上 |
| 14 | 关闭/离开脏保护 | `app.rs:158-165` 窗口 X 拦截；`commands.rs:1346-1399` 确认框 | `tests.rs:341` | ✅ | 缓存 sheet 脏时正确阻止（`commands.rs:1372-1379`）；建议补一条“保存成功后才执行 pending”的回归 |

### 新发现风险排序（均已上报总控；K 系列为总控已知、本轮复核确认行号）

| 级别 | 项 | 一句话 |
|---|---|---|
| 高 | K2/K3 | 单侧替换与 Swap 都会破坏对侧或历史快照的完整性，属于数据丢失/错数据类，须最先修 |
| 中 | K1/K4/K5 | 失败加载卡死、非法设置污染、scan 被取消后状态缺失 |
| 中 | N1 Swap 于加载中卡死（`commands.rs:1211`+`view.rs:471-482`） | 无守卫，Reload 可恢复 |
| 中 | N2 切回缓存 sheet 后 scan 永久停止（`view.rs:539`+`view.rs:368-376`） | 标签色点/Info pending 不恢复 |
| 中 | N3 剪贴板替换复用旧列映射（`view.rs:185-210`） | 校验失败卡加载或静默错配 |
| 中 | N6 查找/替换/导出/摘要在 UI 线程全量同步执行（`commands.rs:772-801`、`summary.rs:62+`、`source.rs:639+`） | 大表上存在阻塞 UI 的**未测风险**（未实测，不断言具体时长）；6.1 帧率基准不覆盖这些交互路径 |
| 低 | N4 swap 后缓存 summaries 口径未反转；N5 Replace 不前进；N7 剪贴板分隔符；N8 导出覆盖无备份；N9 提交编辑后 held_output 报“stale table result discarded”（`view.rs:284-300`，自愈但误导） | 体验/口径类。**N5/N6/N8 不在 4.4 明确验收条款内（自动 Replace 前进、大表同步 Find 耗时、导出覆盖确认均无规格文字），仅记录为风险，不作为 4.4 勾选门禁，也不新增范围或暗暗后置** |

### 回归建议（4.4 勾选前最低集）

1. Swap×历史矩阵：swap 后 Undo/Redo  swap 前编辑（两侧各一）、swap 后 Align With 再 Undo、swap×多 sheet 缓存往返；断言 `is_stale`/revisions/列映射方向。
2. 单侧替换矩阵：Open File/Open Clipboard 替换左/右各一次，对侧 dirty 与 Undo 栈保留；替换失败（不存在文件/非法格式）后视图与历史完整。
3. 加载失败矩阵：初始加载失败、Reload 失败、Settings reload 非法 rules——均不得出现永久 spinner 或污染的 default_rules。
4. 设置应用/取消：仅规则变化不重载且历史保留；options/pairs/paths 变化走确认；取消（关窗）后草稿不残留；手动配对+换文件组合钉住 by_name 重排。
5. sheet 标签：多 sheet xlsx 切回缓存 sheet 后 scan 恢复或状态保留；swap 后 Info 口径正确。
6. 保存/导出：xlsx 侧拒写提示、CSV 导出内容=显示值、备份生成、保存后 `mark_saved` 与缓存 sheet 脏阻止离开。
7. 查找替换：Find Previous 回归；Replace 前进与同步耗时仅为已记录风险（见上），不构成 4.4 门禁。
8. N1 守卫：`swap()` 在 `pair=None`/worker 运行时应拒绝并提示。

## 6.1 覆盖矩阵与同轮 A/B 准备

### 覆盖矩阵（6.1 勾选判定）

| 项 | 命令 / 证据 | 本轮状态 |
|---|---|---|
| Clippy 零警告 | `cargo clippy --workspace --all-targets -- -D warnings` | 待 6.1 同轮重跑（Wave 6 数字不可复用） |
| 全量测试 | `cargo test --workspace` | 同上（Wave 6：303 passed / 0 failed / 3 ignored） |
| 黄金一致率 | `cargo test -p bc-engine-table --test golden_table` ≥95%，KNOWN_DIFFS 不扩大（黄金回归在 bc-engine-table/tests/golden_table.rs，不是 bc-core） | 同上 |
| 表格基准 | `--bench-table` 百万×10，update p99 < 4 ms | 同轮 A/B 待跑 |
| 文本线不退化 | `--bench=600` 双侧各 110,000,043 B，同轮交替 A/B | 同轮 A/B 待跑 |
| 报告 | 本审计 + 实施 worker 修复后的 6.1 结果合并写入 wave 报告 | 待 |

### 基线证据（已核验，详见 `target/table-wave7/audit/environment.txt`）

- **正式基线（可证明）**：`target/table-wave7/baseline-build/release/matcha.exe`，SHA256 `f63125ce…c93`。来源：`git archive e3784b2 → baseline/source`（900 行文件级清单 `audit/source-e3784b2-manifest.sha256`），独立 `CARGO_TARGET_DIR=baseline-build` 构建，`build-e3784b2.log` 尾部 `Finished release` + `exit_code=0`，rustc 1.99.1。
- **参考副本（来源不可证明，不冒充基线）**：`target/table-wave7/baseline/matcha.exe`，SHA256 `de78745c…45b7`，复制自既有 `target/release/`。其 mtime 18:46 早于 `59ca56d` 提交时刻 18:54，确切源版本不可证明；与正式基线同尺寸 13,488,128 B 但哈希不同（debuginfo 在独立 pdb，exe 嵌入构建元数据差异），仅作旁证，A/B 一律使用 baseline-build 版。
- 当前工作树 = `e3784b2` 干净（diff 为空），实施 worker 的定向测试可用同一快照构建候选。

### 输入（同轮 A/B 共用，SHA 已记录，运行前后复验）

- 文本：`target/table-wave7/audit/inputs/text-{left,right}.txt`，自 `tmp/left.txt` / `tmp/right.txt`（各 110,000,043 B，自 11-05 未变）**只读复制**而来，SHA 一致（`b8041ce6…` / `2b2d6627…`）；tmp 原件全程未写入。
- 表格：1,000,000×10，各 69,258,868 B（与 Wave 6 报告输入字节数一致），已确定性生成到 `target/table-wave7/audit/inputs/table-{left,right}.csv`（SHA `1c7f090e…` / `53c033e4…`）；生成器 `table_gui_inputs.exe` 无随机性，可复现。

### 下一轮（6.1 派发后）可执行命令

```powershell
# 说明：新增验证脚本 tools/table_wave7_ab_bench.ps1（仅调用既有二进制与输入，不改源码、
# 不写 tmp/、不重建任何 release），其解析/守卫已由 tools/table_wave7_parser_tests.ps1 离线回归
# （11/11 通过：wave6 真实日志 text p99=1.347 / table p99=1.314 精确读出，缺字段/帧数不足/
# 非 0 退出/超时/轮次不足/A 与 B 同哈希均被拒绝）。同轮交替 A/B，每侧每线 >=3 次，30 预热 + 600 帧。
powershell -NoProfile -ExecutionPolicy Bypass -File tools/table_wave7_ab_bench.ps1 `
    -BaselineExe  target/table-wave7/baseline-build/release/matcha.exe `
    -CandidateExe target/release/matcha.exe `
    -Runs 3
# 产物：target/table-wave7/audit/bench-{text,table}-{A,B}-run<N>.log（raw）、
#       bench-ab-summary.csv（逐轮 update/frame_cpu 分位数）、bench-ab-manifest.json（exe+输入 SHA/HEAD）
# 判定：表格 update p99 < 4 ms；文本线 B 相对 A 同轮 p99 不明显回退（建议阈值 ≤5%，且不超 8 ms CPU p99 预算）。
# 若输入缺失，脚本会用 baseline-build 的 table_gui_inputs.exe 确定性重建并记录 SHA。
```

6.1 之外、本轮**未**做：耗时基准运行、clippy/test/构建（避免与实施 worker 并发争用）、任何实现修改。
