# Tasks

## 1. 文档先行

- [x] 1.1 在 `docs/features/table-compare.md` 增加"Text Details 对照条"条目（列宽覆盖与自动适配、独立水平滚动/滚轮、单击/Shift+单击/列头/双击/右键联动、选区显示、悬停完整文本、清除覆盖的时机），并在代码地图中补 `details.rs` 的新职责；验证：文档条目与 `specs/table-compare-view/spec.md` 五条新需求一一对应

## 2. 对照条布局状态（列宽覆盖 + 偏移）

- [x] 2.1 `table/grid.rs` 的 `GridState` 增加 `details_widths: HashMap<usize, f32>`、`details_offsets`、`details_scroll_x`、`details_follow`、`details_resizing`，并实现 `rebuild_details_offsets()`（覆盖值优先、回退 `widths`）、`details_resize_target(left, x)`、`details_resize_column(d, right)`、`details_follow_cursor(view_width)`（最小滚动、超宽列左对齐、光标列未变不动）、`clear_details_widths()`；验证：新增纯函数单测覆盖命中 ±3 px、最小 20 px/最大 1000 px、后续偏移平移、未覆盖列随 `widths` 变化、跟随与不吸回
- [x] 2.2 在 `project_with_fit(true)`、`select_sheet`、`reload`、`refresh_grid`（列数变化）处调用 `clear_details_widths()`；验证：单测断言 Resize Columns to Fit 与切换 sheet 后覆盖为空（对应 spec "Resize Columns to Fit 清除覆盖"）

## 3. 像素级横向滚动条

- [x] 3.1 `chrome/scrollbar.rs` 抽出横向按钮/轨道/滑块的共享绘制，新增 `show_horizontal_px(ui, rect, content, view, &mut offset)`（按钮步进一个固定像素量、轨道点击/拖动、内容不超宽时滑块填满），`show_horizontal` 改走共享绘制且行为不变；验证：`cargo test -p bc-gui --lib table_merge` 既有横向滚动用例全部通过，并新增像素版单测（拖动到轨道末端时 offset = content - view）

## 4. 对照条交互（details::ui）

- [x] 4.1 `table/details.rs`：`paint` 改为 `ui(...) -> (Option<GridAction>, [Response; 2])`，绘制改用 `details_offsets`/`details_scroll_x`，底部带改为 `show_horizontal_px`，滚轮命中对照条时改写 `details_scroll_x`；验证：harness 测试"滚轮滚动对照条"——对照条上滚轮后 `details_scroll_x` 增大、`grid.scroll` 不变
- [x] 4.2 列头交互：边界悬停/拖拽调整宽度（ResizeHorizontal 光标）、双击边界按当前行两侧文本+列头自动适配（D4）、非边界单击产出活动侧 `Select`；验证：harness 测试"拖拽对照条列边界"（对照条列宽 +N、`grid.offsets` 不变）、"双击边界自动适配"、"单击列头"
- [x] 4.3 行交互：两侧各一个 `interact`，单击 `Select(side, …, shift)`、双击 `Edit`、右键 `Context`；活动侧选中单元格以 `grid::SELECTION` 填充；验证：harness 测试"单击右侧单元格切换焦点"（active=1、cursor 列正确、之后右方向键移到下一列）与"Shift+单击扩展选区"
- [x] 4.4 悬停提示：截断或含换行时 `on_hover_ui` 显示原文（保留换行、4000 字符上限 + 省略号、最大宽 600 px）；验证：harness 测试截断单元格悬停输出中出现完整文本、完整可见单元格不出现提示

## 5. 视图路由与联动

- [x] 5.1 `table/view.rs`：调用 `details::ui`，其动作走 `grid_action`，随后设置 `focus_grid = Some(side)` 并 `ensure_cursor_visible()`；两个行 `Response` 加入 `menu_owners`；每帧在绘制前 `rebuild_details_offsets` + `details_follow_cursor`；验证：harness 测试"双击编辑"（左侧 CSV 打开编辑器、上方网格滚到该格）、"右键菜单"（右键对照条右侧弹出含 Copy to Left 的菜单且命令作用于右侧）、"光标列变化时对照条跟随"、"手动滚动不被吸回"，以及 Align With 待选时单击对照条另一侧完成配对
- [x] 5.2 回归：`cargo test -p bc-gui --lib` 全部通过（重点 `view_tests`、`projection_tests`、`tests.rs` 中依赖对照条文本的用例），并执行 `powershell -ExecutionPolicy Bypass -File tools/dev-build.ps1` 成功；验证：命令输出无失败

## 6. 集成核验与记录

- [x] 6.1 用 debug 版打开两个含长单元格的 xlsx/CSV，人工核验：拖拽/双击边界、滚轮与滚动条、单击两侧切换焦点并方向键继续、右键菜单、悬停提示；截图存 `tmp/` 供 review；验证：截图与 spec 场景逐条对照无偏差
- [x] 6.2 更新 `CHANGELOGS.md`（最新在最上）记录本次对照条交互增强；验证：条目存在且与 feature 文档一致

## Workflow follow-up

- review 通过后执行 `/opsx:archive` 归档，并确认 `openspec/specs/table-compare-view/spec.md` 合入五条新需求。
