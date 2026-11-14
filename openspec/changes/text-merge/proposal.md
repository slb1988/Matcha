# Proposal

## Why

Matcha 目前只支持两方比较，但版本控制工具（首当其冲是 Perforce P4V）解决合并冲突需要三方合并工具：P4V 以 `%1 %2 %b %r`（theirs / yours / base / result）调用外部合并器，并等它退出。仓库中三方合并的骨架（`bc-engine-text::merge`、`Aligner::align3`、`/automerge` 等开关、退出码 0/14/101）已就位但均为 `todo!`，CLI 路由把三方调用挡在 "not supported"。本变更把这条链路打通：能自动合并的自动合并，不能的标为冲突交给用户，GUI 与命令行双模式可用。

## What Changes

- **三方对齐引擎**：实现 `bc-core` 的 `Aligner::align3`（两次双方对齐 + 以 center 为轴调和）与 `bc-engine-text` 的 `Align3Job`；行表增加平行的 center 列，双方比较的行存储不变。`Realign3Job` 以全量重对齐实现。
- **合并模型与自动合并**：实现 `MergeState::classify / conflicts / resolve / render_output` 与 `auto_merge`。逐行分类（未变 / 左改 / 右改 / 同改 / 异改）决定输出内容；冲突按区段判定（两侧不同改动重叠或相距不超过 `conflict_gap` 行）；支持 `/iu`、`/favorleft|right`、`/force`（冲突标记）。
- **CLI 无窗口自动合并**：`/automerge` 在发起进程内执行（仿照 `/qc`），输出目标为第 4 个位置参数或 `/mergeoutput=`；退出码 0 成功、14 有冲突但已带标记写出、101 有冲突未写出、100 错误；`/reviewconflicts` 在有冲突时转入 GUI。
- **GUI 文本合并视图**：新增 Text Merge 标签页——上排三个只读输入窗格（左 / 中 base / 右）+ 底部可编辑输出窗格，按来源着色，冲突导航，Take Left / Center / Right 与"接受当前内容"，保存写出 result 文件。
- **等待与退出码**：合并调用不论走哪条启动路径（转交已有实例 / 成为首个实例 / `/solo`）都让调用方等到合并标签页关闭，并返回 0 / 14 / 101 / 100。P4V 配置 `matcha.exe %1 %2 %b %r` 即可用。
- **参考依据先行**：合并语义逐项对照参考帮助写入 `docs/reference-parity/text-merge.md`，并用参考工具采集自动合并样本，确认 design 中标为"暂定"的语义。
- **表格文本兜底**：CSV/TSV 等文本表格按文本合并处理（按行）。
- **BREAKING（仅命令行返回值）**：`matcha a b c` 与 `matcha a b c d` 原先报 "Text Merge is not supported yet" 并以 100 退出，现在打开合并视图。

不在本变更内：表格 key 列合并、文件夹合并、输入窗格编辑、合并视图内改规则、命名会话与 Home 入口（完整清单见 design.md 的 Non-Goals）。

## Capabilities

### New Capabilities

- `three-way-alignment`: 三个文本序列以 center 为轴的三方对齐算法及其不变量。
- `text-merge-engine`: 行类型分类、冲突区段、默认输出、冲突解决、自动合并结果与输出字节（标记、行尾、编码）。
- `text-merge-cli`: 合并调用的参数归一、`/automerge` 无窗口自动合并、`/reviewconflicts`、GUI 合并的等待与退出码。
- `text-merge-view`: GUI 文本合并视图的窗格布局、着色、冲突解决交互、输出编辑、保存与关闭。

### Modified Capabilities

（无——`openspec/specs/` 尚无既有能力规格；CLI 退出码与位置参数的相关行为以新增 requirement 形式写入 `text-merge-cli`。）

## Impact

- `crates/bc-core/src/align.rs`：三方调和函数；三个策略的 `align3` 委托它。
- `crates/bc-engine-text`：`map.rs`（`AlignedData` center 列、`TextMap` center 访问）、`jobs.rs`（`Align3Job` / `Realign3Job`）、`merge.rs`（全部）、`rules.rs`（`TextMergeRules`：增 `conflict_gap`，删 `conflict_on_center_change`）。
- `crates/bc-script`：`cli.rs`（`merge_spec()`、用法文本）、`runner.rs`（`auto_merge_files`）。
- `apps/bcompare`：`routing.rs`（`Route::AutoMerge`，解除 Merge3/Merge4 的 Unsupported，改写既有单测）、`main.rs`（headless 执行、合并调用隐式等待、以标签页结果码退出）。
- `crates/bc-gui`：新增 `merge/`；`text/render.rs` 窗格绘制抽取；`remote.rs`（`RemoteOpen` 合并载荷、`Outcome` 带结果码）、`app.rs`（`View::TextMerge`）、`lib.rs`（`LaunchOptions`、`run_with_remote` 返回值）、`snapshot.rs`（merge 场景）。
- `tools/golden`：`gen_merge_expected.ps1` 与合并样本语料。
- 文档：新增 `docs/features/text-merge.md`、`docs/reference-parity/text-merge.md`；更新 `docs/features/README.md`、`text-compare.md`、`cli-and-scripting.md`、`crates/bc-engine-text/README.md`、`crates/bc-script/README.md`、`apps/bcompare/README.md`、`apps/bcomp/README.md`、`.agents/skills/matcha-cli/SKILL.md`、`CHANGELOGS.md`。
- 不涉及 `crates/bc-session`（命名会话不在本变更内）。
- 退出码 0/14/101 属参考工具既有约定，CI 与 P4V 集成依赖其精确值。
