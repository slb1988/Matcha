# OpenSpec 剩余事项交接（2026-11-15 归档后）

## 先读这段

- **5 个已完成的 change 已于 2026-11-15 归档**：`text-compare-core`、`folder-compare-sync`、`table-compare-xlsx`、`text-patch-reports`、`hex-compare` → `openspec/changes/archive/2026-11-15-*`；22 个 capability 主 spec 已建立在 `openspec/specs/`（全部纯 ADDED 合并，无 MODIFIED/REMOVED）。
- **table-merge 已于 2026-11-15 归档**（`openspec/changes/archive/2026-11-15-table-merge`，新建主 spec `table-merge-cli`/`-engine`/`-view`，共 25 个主 spec）。
- **text-merge 已于 2026-11-15 带 4 项未完成归档**（用户决定暂不推进；`openspec/changes/archive/2026-11-15-text-merge`，新建主 spec `text-merge-cli`/`-engine`/`-view`、`three-way-alignment`，共 29 个主 spec）。
- **visual-parity 已收口（23/23，待归档）**：按“结构对齐参考、样式走设计系统”（design D9）补齐右键菜单、Session Settings 统一框架、sheet 标签、操作对话框文字化，见其 tasks.md「收口记录」。
- table-merge 已收口（见 §1）；text-merge 剩的全是验证类（2 项被参考工具阻塞）；visual-parity 剩的全是用户明确后置的范围。
- 工作目录 `D:\Github\Matcha`，分支 `master`。归档操作本身未单独提交（openspec 文件移动 + 本文档 + CHANGELOGS 一条），随下一次代码提交或用户指示处理。

## 1. table-merge（25/25，已归档）

**2026-11-15 更新**：原交接表里的 4.1–4.5、3.4、3.5 其实已随 `0bdc1e3`/`dde6cec`/`c0a2863` 落地（tasks 当时未勾）。
本轮核对 spec 后补齐：删除输出行（spec「输出编辑」，引擎 `SheetMerge::remove_row` + `Ctrl+Delete`）、横向滚动整列对齐、
app 层/真实 exe 转交/路由/快照测试；spec 与已落地行为对齐（改动区段导航、模板写出、P4V 参数顺序）；
text-merge-cli 的"一律按文本合并"已直接在 text-merge delta 里改为按格式分派——**archive 顺序问题已消除**。
5.4 全量门禁见 tasks.md 验证记录。已归档，无后续。

## 2. text-merge（30/34，已归档，剩余验证项搁置）

**2026-11-15**：用户决定暂不推进，带下表 4 项未完成归档（见归档目录 tasks.md「归档说明」）。下表保留作日后重启参考。

引擎、CLI、GUI、等待/退出码全部完成且测试通过，7.1 全量门禁已过。剩 4 项全是验证类：

| 任务 | 状态 | 解除条件 |
|---|---|---|
| 1.3 持锁运行参考工具采集合并样本 | ⛔ 阻塞 | 本机 BC 5.0.6（`D:\Program Files\Beyond Compare 5\BCompare.exe`）三种调用方式（`left right center out`、`left right center`、`/fv="Text Merge" …`）都只打开 Text Compare 窗口而非 Text Merge，输出文件不产生；推断本机许可不含 Pro 三方合并。**解除：换用能打开 Text Merge 的参考环境，重跑 `tools/golden/gen_merge_expected.ps1`**，然后把 P1–P6、P8 结论写入 `docs/reference-parity/text-merge.md` |
| 3.5 黄金回归测试 | ⛔ 阻塞（依赖 1.3） | `crates/bc-engine-text/tests/golden_merge.rs` 骨架已就位（样本缺失报错不跳过）；样本入库后跑 `cargo test -p bc-engine-text`，差异按 `KNOWN_DIFFS` 登记。目前 20 用例 × 4 组合固定的是暂定语义，非参考对齐 |
| 7.2 P4V 端到端（人工） | 待人工 | 需 P4V + 真人窗口。六种组合：`matcha.exe %1 %2 %b %r` ×（无实例/有实例）×（解决保存/不保存关闭），再 `/automerge /reviewconflicts` ×（无冲突/有冲突）。自动化证据已有：`apps/bcompare/tests/hand_off.rs`（真实 exe 验证等待与 0/14/101/100）、`app_tests.rs`（首实例结果码）。未验证：P4V 以什么判定完成、`%r` 是否预先存在、参考工具 P7 |
| 7.3 端到端冒烟 | 部分完成 | 无冲突自动合并、CSV 按文本合并已由真实 exe/runner 测试覆盖；差**真实窗口手工走一遍冲突 GUI 解决**（现只有无头测试与快照 `merge-basic`/`merge-conflict` 两张 PNG） |

**Feature 文档状态**：`docs/features/text-merge.md` 已写明"未经参考样本确认"，与 1.3 阻塞注记一致，解除阻塞后需同步更新。

## 3. visual-parity（23/23，待归档）

**2026-11-15**：已按 design D9 收口，下文为收口前的状态记录。

核心版已收口（7.1 报告 `docs/reference-parity/visual.md`，429 tests 全绿），follow-up 明确**不归档，等用户决定后置范围**。剩余 8 项三组：

- **4.2 / 4.3**：文件夹同步视图外观 + 操作确认对话框；文件夹场景对照达标合回 master（改动范围限 `dir_view.rs`/`sync_view.rs`/`filters_bar.rs` 与 `app.rs` 最小接线）。
- **5.1–5.3**（2026-11-10 用户收窄"抓大放小"后置）：Home 页（会话树/新会话网格/工具栏菜单）、表格比较全套外观、对照达标且 `--bench-table` 不退化。
- **6.1–6.3**：对话框（文本/文件夹/表格会话设置、Options、Find/Replace、File Formats、Go To 的尺寸/页签/控件布局/按钮行）、右键菜单（条目与顺序、阴影/边框/高亮色）、对照达标。注意：参考工具 Find/Replace 实为停靠查找条而非对话框（tokens 场景 `text-find-bar`）。

**注意（2026-11-15 实况核对）**：归档前 master 上刚完成"界面风格统一（组件库重构）"（见 CHANGELOGS 2026-11-15 条目）——工具栏下拉、浮窗、按钮/勾选框/输入框已统一为设计系统控件（抹茶绿强调色，`theme::Palette` 见 ADR 402），Hex 对话框文案英文化。**2026-11-15 用户已决定：以现代风设计系统为准（抹茶绿是其强调色），不做经典 Windows 1:1 复刻**；重启各组时只对照参考工具的信息结构（页签、控件、条目顺序、按钮行），视觉样式走设计系统。已写入 visual-parity tasks.md。

## 归档后续

- 主 specs 与归档结果一致性已随 `openspec archive` 内建验证通过；`openspec list` 确认活跃 change 仅剩 3 个。
- `text-merge-cli` 的按格式分派已写进 text-merge delta，两者归档无顺序依赖；visual-parity 归档与否等用户定范围。
