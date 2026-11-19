# Proposal

## Why

文本合并视图已能完成三方合并，但审阅体验明显落后于文本比较视图：改动行只有整行底色，看不出行内具体哪几个字符不同；没有语法着色；不能查找替换；没有横向滚动条（只能 Shift+滚轮）；输入窗格既不能选择也不能复制。解冲突时用户往往要切到别的工具对照，这是合并视图最影响日常使用的一组缺口（见 `docs/features/text-merge.md`「尚未实现」）。

## What Changes

- **行内差异高亮**：左、右输入窗格的改动行与 base（中间窗格）同一对齐行逐字符比较，不同的字符用所属侧的强调色着墨（左侧改动色 / 右侧改动色，冲突区段用冲突色）；不重要差异沿用文本比较的不重要差异色。
- **语法着色**：四个窗格按文件格式库的语法着色，风格与文本比较一致；新增 View 菜单的 Syntax Highlighting 开关（默认开）。
- **查找与替换**：Search 菜单新增 Find…（Ctrl+F）、Find Next（F3）、Find Previous（Shift+F3）、Replace…（Ctrl+H），复用文本比较的查找条（字面 / 正则、大小写、全词、仅在差异中）。查找作用于活动窗格；替换只对输出窗格可用，可撤销，被替换的行标为手工。
- **横向滚动条**：任一窗格内容超宽时，在窗格区底部显示一条四窗格共用的横向滚动条。
- **输入窗格选择与复制**：三个输入窗格支持单击定位、拖选、Shift+单击扩展选区、Ctrl+A 全选、Ctrl+C 复制；新增输入窗格右键菜单（Take Left / Center / Right、Copy、Select All）。输入窗格仍然只读。

## Capabilities

### New Capabilities
（无）

### Modified Capabilities
- `text-merge-view`: 新增行内差异高亮、语法着色、查找替换、横向滚动条、输入窗格选择与复制五项需求；既有需求（四窗格布局只读、着色、Take、保存）不变。

## Impact

- `crates/bc-gui/src/merge/view.rs`：窗格绘制传入语法 token 与行内差异区间；输入窗格选区与右键菜单；查找条接入；横向滚动条；菜单新增 View 与 Search 项、快捷键。
- `crates/bc-gui/src/merge/model.rs`：输出窗格的替换走模型编辑入口以维护来源标记；按侧 / 行查找对齐行。
- 复用 `crates/bc-gui/src/text/find_panel.rs`、`text/render.rs::layout`、`bc-engine-text::{inline, search}`、`chrome::scrollbar::show_horizontal_px`；不改引擎语义，无新依赖。
- 不涉及命令行、退出码与表格合并视图。
- 文档：`docs/features/text-merge.md`（功能简介、关键语义、从「尚未实现」移除这五项）与 `CHANGELOGS.md`。
