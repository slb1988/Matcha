# Design

## Context

- `crates/bc-gui/src/text/gutter.rs::paint` 只画行号和箭头，在 `importance != Same` 且本侧有行时画一个黄色多边形箭头，没有交互。
- `render.rs::pane` 用 `allocate_exact_size(.., click_and_drag)` 分配整行（含行号栏），然后用**原始指针输入**（`primary_pressed` / `primary_down` + `viewport_rect.contains(p)`）放置光标、开始拖选、处理 align-with。点在箭头上会被当成点在行首文本，所以箭头"点不动"。
- 已有复制逻辑（`view.rs`）：
  - `TextMap::copy_section(row, from)`：整块复制，返回 `EditItem`，`copy_to` 无选区分支在 push 后调用 `after_alignment()`（同步结果，不需要等 worker）。
  - `copy_lines(destination, lines)`：按来源行区间替换目标侧对齐行，push `LineEdit::replace_range` 后调用 `recompare()`（异步）。
  - `TextContent::ensure_writable()` 给出只读等原因；`map.is_stale()` 表示编辑后对齐结果已过期。
- 渲染循环中途不能修改 `map` / `projection`：行号、投影行在本帧剩余部分还要用。

## Goals / Non-Goals

**Goals:**
- 箭头成为独立命中区，点击触发复制，不影响文本光标、拖选、双击和右键。
- 区块箭头与选区箭头外观可以区分，并有可用、悬停、禁用三种状态。
- 复用现有复制路径，撤销粒度和重新对齐行为与菜单命令一致。

**Non-Goals:**
- 文本合并（三方）视图的箭头；表格/十六进制视图。
- 空隙侧（本侧没有行）的箭头，也就是"用本侧的空覆盖对侧、删除对侧行"；这种情况仍用菜单 Copy to Left/Right 处理。
- 隐藏行号（`show_numbers == false`）时另外提供箭头列；行号栏隐藏时箭头一起隐藏，保持现状。
- 键盘操作箭头（快捷键 Ctrl+R/L、Shift+Ctrl+R/L 已覆盖）。

## Decisions

1. **决定箭头的纯函数**：在 `TextCompareView` 上新增 `gutter_arrow(s, row) -> Option<GutterArrow>`，`GutterArrow { kind: Section | Selection, disabled: Option<String> }`。
   - `Selection`：side `s` 的 editor 有非空选区，且该行的本侧行号落在 `selected_lines` 内（无论重要性）。
   - `Section`：否则，本侧有行、`row.section.is_some()`，且区块重要性（计入 ignore_unimportant）不是 `Same`。这里改用 section 判断，而不是只看行的 `importance`，因为 `copy_section` 要求行属于某个区块。
   - `disabled`：`map.is_stale()` 时给出 "Comparison is updating"；目标侧只读时给出 "Right/Left side is read-only"。
   - 对齐过期期间 `row()` 退化为逐行并排、没有 section/重要性（窗格本身也不着色），因此区块箭头不显示，只剩禁用的选区箭头；对齐完成后恢复。
   - 选区行区间每帧每窗格只算一次，再传进行循环，不按行重复计算。
   - 理由：可以脱离 egui 单测，`gutter::paint` 也只管绘制。

2. **命中区先注册、再绘制**：`gutter::arrow_rect(viewport_rect)` 给出固定命中矩形（行号栏左侧 20px × 行高）。行循环里，在行 `allocate_exact_size` 之后用 `ui.interact(arrow_rect, id(("gutter_arrow", s, row)), Sense::click())` 注册，这样它在行响应之上。`hovered()` 再传给 `gutter::paint` 决定高亮。
   - 备选方案是在 `paint` 之后再判断命中，但那样拿不到悬停状态用于绘制，也挡不住同帧的原始指针处理，所以不采用。

3. **原始指针处理排除箭头区**：行循环里现有的 `pointer.filter(...)` 增加 `!arrow_rect.contains(p)`（仅当该行有箭头时），双击分支也一样跳过。这样点击箭头不会放光标、不会设 `dragging`、不会消费 align-with。窗格级 `response.clicked()` 被上层箭头控件挡住，不会触发。

4. **延迟执行**：点击时只记录 `self.pending_gutter_copy = Some((s, row, kind))`，在 `pane()` 的 ScrollArea 结束后（右键菜单处理附近）统一执行 `apply_gutter_copy`。这样本帧的行循环不会看到改了一半的 map/projection。
   - `Section` → 提取 `copy_section_at(destination, row)`（`copy_to` 无选区分支也改为调用它）：`copy_section(row, side(s))` → push 到目标侧 → `after_alignment()`。
   - `Selection` → `copy_lines(1 - s, selected_lines)`，然后清除 `editors[s].anchor`（选区箭头随之消失），光标留在原处。
   - 执行前再次检查 disabled 条件（防止点击和执行之间状态变化）；失败时写 `self.message`，不 panic。
   - 成功时写 `self.message`，如 "Copied section to Right" / "Copied 2 line(s) to Right"。

5. **外观**：
   - 区块箭头：维持现有黄色 `(255,220,110)` 多边形；悬停时填充加深并加 1px 描边。
   - 选区箭头：同一箭头形状改用主题强调蓝色填充，箭头尖端外加一条 1.5px 竖线（"→|" 表示"覆盖到这一侧"），一眼就能和区块箭头区分。
   - 禁用：两种箭头都改为灰色填充，不高亮，不用手形光标。
   - 悬停提示用 `on_hover_text`："Copy section to Right/Left"、"Copy selected lines to Right/Left"，禁用时显示原因；可用时 `CursorIcon::PointingHand`。

## Risks / Trade-offs

- [箭头命中区和行首文本相邻，用户想点第一列字符却点到箭头] → 命中区限定在行号栏内（文本起点在 `gutter::WIDTH + 1`），不侵入文本区。
- [`copy_lines` 走异步 `recompare`，复制后短时间内 map 处于过期状态，箭头为灰色] → 这是预期行为（spec 的"编辑后对齐尚未完成"场景）；`Section` 路径同步 `after_alignment`，不受影响。
- [每行多注册一个 interact，大文件滚动开销增加] → 只对有箭头的可见行注册（行循环本来就只处理可见行），开销与可见行数成正比；用已有 `last_stats.rows_drawn` 性能测试回归。
- [选区跨越空隙行时 `selected_lines` 和对齐行的映射有歧义] → 沿用 `copy_lines` 现有语义（与 Copy Line to Other 一致），不另立规则。

## Open Questions

- 选区箭头的蓝色是否直接取 `chrome::theme` 的强调色，还是单独定一个常量：实现时按主题现有色板选，不影响行为。
