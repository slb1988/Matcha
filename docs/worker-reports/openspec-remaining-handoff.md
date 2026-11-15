# OpenSpec 剩余事项交接（2026-11-15 归档后）

## 先读这段

- **5 个已完成的 change 已于 2026-11-15 归档**：`text-compare-core`、`folder-compare-sync`、`table-compare-xlsx`、`text-patch-reports`、`hex-compare` → `openspec/changes/archive/2026-11-15-*`；22 个 capability 主 spec 已建立在 `openspec/specs/`（全部纯 ADDED 合并，无 MODIFIED/REMOVED）。
- **剩 3 个活跃 change，共 20 项未完成**：`table-merge`（17/25）、`text-merge`（30/34）、`visual-parity`（15/23）。
- **唯一能纯代码推进的是 table-merge 的 GUI 表格合并视图**；text-merge 剩的全是验证类（2 项被参考工具阻塞）；visual-parity 剩的全是用户明确后置的范围。
- 工作目录 `D:\Github\Matcha`，分支 `master`。归档操作本身未单独提交（openspec 文件移动 + 本文档 + CHANGELOGS 一条），随下一次代码提交或用户指示处理。

## 1. table-merge（17/25，建议主攻）

**已完成**：三方键对齐引擎、合并模型与 `auto_merge`、CSV 保真/xlsx 新写（含追加的 `xlsx_patch` 模板写出 + `formula_refs` 行引用重定位，见 design D3 二次修订）、CLI 大部分开关、golden 语料 24 例（5.1）、10 万行性能验证（5.2）、feature 文档（5.3）。

**剩余 8 项，建议顺序**：

| 序 | 任务 | 内容 |
|---|---|---|
| 1 | 4.1 | 新建 `crates/bc-gui/src/table_merge/model.rs`：四网格状态模型、按 sheet 懒建 MergeState、切换不丢解决进度、结果码接口 |
| 2 | 4.2 | 视图渲染：复用 `table/` 网格（虚拟滚动/着色/sheet tabs），上排 left/center/right 只读 + 底部输出网格，空隙空白、冲突色 |
| 3 | 4.3 | 冲突导航（下一个/上一个区段、计数）+ Take 交互（格/行/区段取 left/right/center、接受当前、删改取删除侧移除行）；输出格编辑视为解决 |
| 4 | 4.4 | 保存与关闭：暂存+rename 写出、遗留冲突允许保存（结果码 14）、脏关闭提示、从未保存关闭结果码 101 |
| 5 | 4.5 | 接入 `app.rs`（`View::TableMerge`、标签页标题/图标/脏状态）与 `snapshot.rs` |
| 6 | 3.4 | `/reviewconflicts` 有冲突时转 GUI 并携带 headless 已应用状态（favor/键），无冲突不开窗；routing/main 层测试对照 text-merge 同名场景 |
| 7 | 3.5 | GUI 合并的等待与结果码：`View::TableMerge` 接入既有转交/等待/Outcome 管道，退出码 0/14/101/100；`hand_off` 测试覆盖已有实例转交仍等待 |
| 8 | 5.4 | 全量门禁：`cargo test` 全绿 + `tools/dev-build.ps1` 通过 |

**注意**：
- 3.4/3.5 依赖 4.x 视图就位，故排在后。
- **Archive 顺序对齐**（tasks.md Workflow follow-up）：若 text-merge 先 archive，本变更 archive 前需补 `text-merge-cli`「一律按文本合并处理」requirement 的 MODIFIED delta（按格式分派）；反之在 text-merge archive 时处理（见 design.md D4/Risks）。两者目前都未归档，谁先收口谁带上这个 delta。
- 参考实现：text-merge 的 GUI 视图（`crates/bc-gui/src/merge/`）是结构模板，等待/退出码管道直接复用。

## 2. text-merge（30/34，代码全绿，剩验证）

引擎、CLI、GUI、等待/退出码全部完成且测试通过，7.1 全量门禁已过。剩 4 项全是验证类：

| 任务 | 状态 | 解除条件 |
|---|---|---|
| 1.3 持锁运行参考工具采集合并样本 | ⛔ 阻塞 | 本机 BC 5.0.6（`D:\Program Files\Beyond Compare 5\BCompare.exe`）三种调用方式（`left right center out`、`left right center`、`/fv="Text Merge" …`）都只打开 Text Compare 窗口而非 Text Merge，输出文件不产生；推断本机许可不含 Pro 三方合并。**解除：换用能打开 Text Merge 的参考环境，重跑 `tools/golden/gen_merge_expected.ps1`**，然后把 P1–P6、P8 结论写入 `docs/reference-parity/text-merge.md` |
| 3.5 黄金回归测试 | ⛔ 阻塞（依赖 1.3） | `crates/bc-engine-text/tests/golden_merge.rs` 骨架已就位（样本缺失报错不跳过）；样本入库后跑 `cargo test -p bc-engine-text`，差异按 `KNOWN_DIFFS` 登记。目前 20 用例 × 4 组合固定的是暂定语义，非参考对齐 |
| 7.2 P4V 端到端（人工） | 待人工 | 需 P4V + 真人窗口。六种组合：`matcha.exe %1 %2 %b %r` ×（无实例/有实例）×（解决保存/不保存关闭），再 `/automerge /reviewconflicts` ×（无冲突/有冲突）。自动化证据已有：`apps/bcompare/tests/hand_off.rs`（真实 exe 验证等待与 0/14/101/100）、`app_tests.rs`（首实例结果码）。未验证：P4V 以什么判定完成、`%r` 是否预先存在、参考工具 P7 |
| 7.3 端到端冒烟 | 部分完成 | 无冲突自动合并、CSV 按文本合并已由真实 exe/runner 测试覆盖；差**真实窗口手工走一遍冲突 GUI 解决**（现只有无头测试与快照 `merge-basic`/`merge-conflict` 两张 PNG） |

**Feature 文档状态**：`docs/features/text-merge.md` 已写明"未经参考样本确认"，与 1.3 阻塞注记一致，解除阻塞后需同步更新。

## 3. visual-parity（15/23，用户收窄后置，等范围决定）

核心版已收口（7.1 报告 `docs/reference-parity/visual.md`，429 tests 全绿），follow-up 明确**不归档，等用户决定后置范围**。剩余 8 项三组：

- **4.2 / 4.3**：文件夹同步视图外观 + 操作确认对话框；文件夹场景对照达标合回 master（改动范围限 `dir_view.rs`/`sync_view.rs`/`filters_bar.rs` 与 `app.rs` 最小接线）。
- **5.1–5.3**（2026-11-10 用户收窄"抓大放小"后置）：Home 页（会话树/新会话网格/工具栏菜单）、表格比较全套外观、对照达标且 `--bench-table` 不退化。
- **6.1–6.3**：对话框（文本/文件夹/表格会话设置、Options、Find/Replace、File Formats、Go To 的尺寸/页签/控件布局/按钮行）、右键菜单（条目与顺序、阴影/边框/高亮色）、对照达标。注意：参考工具 Find/Replace 实为停靠查找条而非对话框（tokens 场景 `text-find-bar`）。

**注意（2026-11-15 实况核对）**：归档前 master 上刚完成"界面风格统一（组件库重构）"（见 CHANGELOGS 2026-11-15 条目）——工具栏下拉、浮窗、按钮/勾选框/输入框已统一为设计系统控件（抹茶绿强调色，`theme::Palette` 见 ADR 402），Hex 对话框文案英文化。这与 visual-parity 第 6 组"经典 Windows 1:1 复刻"方向存在偏差，重启该组任务前需与用户确认以哪套外观为准。

## 归档后续

- 主 specs 与归档结果一致性已随 `openspec archive` 内建验证通过；`openspec list` 确认活跃 change 仅剩 3 个。
- text-merge / table-merge 收口归档时，按上文"Archive 顺序对齐"处理 `text-merge-cli` 的 MODIFIED delta；visual-parity 归档与否等用户定范围。
