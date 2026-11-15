# Proposal

## Why

表格比较底部的 Text Details 对照条（当前行两侧的两行小网格，`crates/bc-gui/src/table/details.rs`）目前**只绘制、不响应任何鼠标操作**：列宽与水平滚动完全借用上方网格，底部 16 px 带只是占位符。长单元格（如 `/Script/PLCoreGame.PhysicalMaterialWithTags'/Game/005_Ma…`）在对照条里同样被截断，用户既不能拉宽列查看，也不能点击单元格切换焦点。对照条本来就是"看清当前行"的地方，这些交互缺失让它失去主要价值。

## What Changes

- 对照条列头边界支持**拖拽调整列宽**（悬停水平调整光标、最小 20 px），双击边界按当前行两侧内容**自动适配**该列宽度。
- 对照条列宽为**独立覆盖值**：未调整的列沿用上方网格列宽；在对照条里调整只影响对照条，不改上方网格（上方网格调整列宽时，未被对照条覆盖的列同步变化）。`Resize Columns to Fit`、切换 sheet、重新加载会清除覆盖值。
- 对照条拥有**独立水平滚动**：底部占位带变为真实横向滚动条；鼠标滚轮（纵向/横向）在对照条上滚动对照条；光标列变化（任意来源）时对照条最小滚动使光标列可见。
- **点击单元格切换焦点并与上方联动**：单击对照条某侧某列 → 活动侧切到该侧、光标移到（当前行，该列），键盘焦点交给该侧上方网格，上方网格横向滚动使该格可见；Shift+单击在当前行内扩展选区；单击列头选中活动侧该列的当前行单元格。
- 对照条行内**显示选区**：上方网格选区覆盖当前行的单元格在对照条活动侧同样以选中色绘制。
- **双击单元格进入编辑**（等同上方网格的 Modify，编辑器出现在上方网格中，只读侧沿用现有提示）。
- **右键菜单**：右键对照条单元格先按上方网格规则选中所点格，再弹出与该侧网格相同的上下文菜单。
- **悬停提示完整内容**：单元格文本被截断或含换行时，悬停显示完整文本（保留换行，长度/宽度有上限）。

非目标：对照条内字符级差异高亮、对照条高度可拖动/自动换行、单元格内部分文本选择、列宽写入会话文件。

## Capabilities

### New Capabilities

（无）

### Modified Capabilities

- `table-compare-view`：新增"Text Details 对照条交互"需求（列宽调整与自动适配、独立水平滚动、点击/右键/双击与上方网格联动、选区显示、悬停完整文本）。

## Impact

- 代码：`crates/bc-gui/src/table/details.rs`（由纯绘制改为绘制+交互，返回 `GridAction`）、`table/view.rs`（布局、动作路由、菜单归属、滚轮）、`table/grid.rs`（`GridState` 增加对照条列宽覆盖与水平偏移）、`table/commands.rs`（复用 `grid_action` / `ensure_cursor_visible`，`Resize Columns to Fit` 清除覆盖）、`chrome/scrollbar.rs`（像素级横向滚动条）。
- 测试：`crates/bc-gui/src/table/view_tests.rs` 或新增 `details_tests.rs`。
- 文档：`docs/features/table-compare.md`、`CHANGELOGS.md`。
- 无引擎、CLI、文件格式、会话格式变化；无新依赖。
