# Proposal

## Why

文本比较视图的行号栏（gutter）在差异行旁画了黄色左右箭头（左侧窗格 ➡、右侧窗格 ⬅），视觉上暗示"点击把这一侧覆盖到另一侧"，但目前 `gutter::paint` 只负责绘制、没有任何命中处理，点击会落到行的文本点击逻辑里，仅仅移动光标——用户看到箭头却点不动，只能去找菜单或记 Ctrl+R / Ctrl+L。参考工具（Beyond Compare）里这些箭头就是一键合并入口，且在有选区时只复制选中行，这是对比后快速合并最常用的操作。

## What Changes

- **区块箭头可点击**：差异区块中带本侧行的行号栏箭头变为可点击按钮；点击把**整个差异区块**从本侧复制到另一侧（覆盖对侧对应内容，整段自动合并），一次可撤销编辑，随后重新对齐。底层复用已有的 `TextMap::copy_section`。
- **选区箭头（新图标）**：当某侧存在选区时，被选中各行的行号栏改画另一种外观的"选区箭头"（与黄色区块箭头明显区分，例如蓝色、箭头尖端带竖线），对选区内的相同行也显示；点击把**选中的行**覆盖到另一侧与之对齐的行（无对齐行时插入到对应位置），一次可撤销编辑。底层复用已有的 `copy_lines`（即 Copy Line to Other 的语义）。
- **命中与反馈**：箭头命中区独立于文本行，点击箭头不移动光标、不开始拖选；悬停时显示手形光标、高亮箭头并给出提示（如 "Copy section to Right" / "Copy selected lines to Right"）。
- **不可用状态**：目标侧只读、或对齐结果过期（编辑后重算中）时箭头画成灰色、不可点击，悬停提示原因。
- 完成复制后清除来源侧选区（选区箭头随之消失），状态栏消息报告结果。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `text-compare-view`：修改"区块复制到另一侧"需求，加入行号栏区块箭头与选区箭头两种点击入口，以及只读/过期时的禁用行为。

## Impact

- 代码：`crates/bc-gui/src/text/gutter.rs`（绘制两种箭头 + 返回命中矩形/状态）、`crates/bc-gui/src/text/render.rs`（行内为箭头注册独立交互、文本按下逻辑排除箭头命中区）、`crates/bc-gui/src/text/view.rs`（新增箭头点击处理入口，复用 `copy_section` / `copy_lines`）、`crates/bc-gui/src/text/view_tests.rs`（原始输入点击测试）。
- 引擎 `bc-engine-text` 不改 API。
- 文档：`docs/features/text-compare.md` 增加"行号栏复制箭头"说明；`CHANGELOGS.md`。
- 不涉及：文本合并（三方，`text-merge-view`）视图的箭头、表格/十六进制视图、CLI。
