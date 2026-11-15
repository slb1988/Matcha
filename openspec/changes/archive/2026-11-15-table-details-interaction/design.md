# Design

## Context

- `table/details.rs::paint` 只用 painter 绘制：列偏移直接用 `GridState.offsets`，水平位置用上方网格的 `scroll.x`，没有任何 `ui.interact`；底部 16 px 带画一个 "—" 占位符。
- 上方网格的交互集中在 `grid::pane`：列头边界拖拽（`resize_target` / `resize_column`，共享 `widths`）、单元格点击产出 `GridAction::{Select, Edit, Context}`，由 `TableCompareView::grid_action` 统一处理（含 Align With、右键保留选区、Modify），右键菜单由 `view.rs` 中的 `menu_owners` + `context_items(side)` 弹出。
- 键盘焦点通过 `focus_grid = Some(side)` → `restore_grid_focus` 交给 `("table_grid_focus", side)`；`ensure_cursor_visible` 负责上方网格滚动（水平为左对齐约定）。
- 滚轮由 `grid::wheel` 按 `pane_rects` 命中消费，不覆盖对照条区域。
- 列宽重置点：`project_with_fit(true)`（Resize Columns to Fit）、`refresh_grid` 列数变化、`select_sheet`、`reload`。

需求见 `specs/table-compare-view/spec.md`。

## Goals / Non-Goals

**Goals:**
- 对照条交互全部复用上方网格的动作通道（`GridAction` → `grid_action`），不新增第二套选择/编辑/菜单逻辑。
- 对照条布局状态（列宽覆盖、水平偏移）与上方网格状态解耦，但列集合（Hide Same Columns 后的 `columns`）共用。

**Non-Goals:**
- 不改上方网格的行为与度量；不改对照条高度（仍为 81 px）。
- 不把对照条做成可获得键盘焦点的独立控件（键盘始终交给上方网格）。

## Decisions

### D1 列宽：独立覆盖表，而非共享上方列宽
`GridState` 新增 `details_widths: HashMap<usize, f32>`（比较列 → 宽度）与 `details_offsets: Vec<f32>`（每帧按 `columns` + 覆盖/回退宽度重算，代价 O(可见列数) 可忽略）。拖拽/双击只写覆盖表。

- 备选：直接写 `GridState.widths`（与上方共享）。实现最省，但用户在对照条拉宽一个长单元格会把上方网格同一列一起撑宽，破坏主网格的浏览布局；对照条只有一行，"为看清这一格而拉宽"是主要场景，独立更符合目的。
- **review 决策点**：若更希望共享（上下拖哪都一起变），只需删去覆盖表、改用 `resize_column`，spec 中"MUST NOT 改变上方网格列宽"与"未覆盖列跟随"两条场景需相应改写。

覆盖表在 `project_with_fit(true)`、`select_sheet`、`reload` 时清空；`refresh_grid` 列数变化时清空。

### D2 水平滚动：像素偏移 + 像素版横向滚动条
`GridState` 新增 `details_scroll_x: f32` 与 `details_follow: Option<usize>`（上次跟随的光标列）。每帧绘制前：若 `cursor.column != details_follow`，按最小滚动量使光标列可见（列宽超过视口时左对齐），然后记录；否则保持用户位置。滚动上限按 `details_offsets.last() - 视口宽` 夹紧。

- 底部 16 px 带改为真实滚动条。`chrome::scrollbar::show_horizontal` 以"首列索引"为单位（表格合并在用），对 600 px+ 宽列无法看到列尾，因此在 `chrome/scrollbar.rs` 增加像素版 `show_horizontal_px(ui, rect, content, view, &mut offset)`，外观与按钮/轨道/滑块逻辑与现有版本一致（抽出共享绘制函数，避免复制）。像素版函数本身在内容不超宽时滑块填满轨道；对照条只在内容超宽时调用它，不超宽时保留原参考外观（面板色带 + "—"），带区高度不变。
- 滚轮：对照条 rect 上的 `crate::chrome::wheel::delta` 纵向/横向分量均映射到 `details_scroll_x`；由于 `grid::wheel` 只命中 `pane_rects`，两者天然不冲突。
- 备选：继续跟随上方 `scroll.x`。D1 后两边偏移表不同，跟随会错位，故放弃。

### D3 交互：`details::ui` 返回 `Option<GridAction>`
`details::paint` 改为 `details::ui(ui, rect, map, &mut GridState, different, colors) -> (Option<GridAction>, [egui::Response; 2])`：

- 列头区一个 `interact`（resize 命中复用与 `resize_target` 相同的 ±3 px 规则，但基于 `details_offsets` 与 `details_scroll_x`，抽成纯函数以便单测）；`drag` → 写覆盖；`double_clicked` 命中边界 → 自动适配；非边界单击 → `GridAction::Select(state.active, Cursor{row: cursor.row, column: c}, false)`。
- 两侧各一个行 `interact`（每侧 18 px 行），点击 → `Select(side, …, shift)`，双击 → `Edit(side, …)`，右键 → `Context(side, …)`。两个 `Response` 交给 `view.rs` 的 `menu_owners`，从而复用同一 `context_items(side)` 菜单。
- `view.rs` 路由：对照条动作与网格动作同样走 `grid_action`；额外设置 `focus_grid = Some(side)` 并调用 `ensure_cursor_visible()`（让上方网格滚到该格）。Align With 由 `grid_action` 既有分支自然处理。
- 选区显示：活动侧单元格若 `selection_visible && state.selected(cursor.row, c)` 用 `grid::SELECTION` 填充，与上方一致。

### D4 自动适配宽度
双击边界时测量当前行两侧该列 `display_at` 文本（换行替换为 ` ↵ `，与绘制一致）及列头字母，用 `ctx.fonts(|f| f.layout_no_wrap(...))` 取宽度 + 12 px 内边距，夹在 `[MIN_COLUMN_WIDTH, 1000]`。只看当前行，符合"看清这一格"的目的且 O(1)。

### D5 悬停提示
行 `Response` hover 时定位单元格；当 `layout 宽度 > 单元格宽 - 12` 或原文含 `\n`/`\r` 时，`on_hover_ui` 显示原文（保留换行，最多 4000 字符 + "…"，提示最大宽 600 px 自动换行）。完整可见时不弹，避免干扰。

## Risks / Trade-offs

- [对照条与上方同列宽度不一致，可能让用户困惑] → 覆盖仅在用户主动操作后产生，`Resize Columns to Fit` 一键恢复；feature 文档写明。
- [`details_follow` 跟随逻辑与手动滚动的竞争] → 只在光标列"变化"那一帧跟随一次，手动滚动之后不再吸回（spec 场景覆盖）。
- [右键菜单 Response 增多，`context_menu` 对重叠 rect 的处理] → 对照条与网格 rect 不重叠；测试覆盖"右键对照条右侧弹出菜单且命令作用于右侧"。
- [悬停时每帧 layout 长文本] → 仅对 hover 的一个单元格 layout，egui 有 galley 缓存。

## Migration Plan

纯 GUI 行为增强，无数据/会话格式变化；回滚即还原 `details.rs`、`view.rs`、`grid.rs`、`scrollbar.rs` 相关改动。

## Open Questions

- 对照条列宽覆盖是否需要写入会话文件（当前上方网格列宽同样不持久化）——可在后续变更中与上方网格列宽持久化一并决定，不影响本次实现。
