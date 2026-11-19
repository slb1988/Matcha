# Design

## Context

- 文本比较菜单（`text/menu.rs`）已有 Toggle Bookmark ▸ Bookmark 0–9（Shift+Ctrl+0–9）、Go to Bookmark ▸ Bookmark 0–9（Ctrl+0–9）、Clear Bookmarks，`update_menus` 的启用白名单里没有它们，所以恒灰；`command()` 也没有对应分支。
- 文本合并视图（`merge/view.rs`）自建菜单，Search 只有 Next/Previous Change Section；行模型为对齐行 `row`，输入窗格只读，输出编辑不改变 `row` 编号。
- 编辑入口分散（键入、粘贴、区块 / 选区复制、行号栏箭头、Replace All、换行符转换、缩进、Undo/Redo），统一的后置钩子 `TextCompareView::edited(old_line_count)` 只知道行数变化，不知道位置。
- `TextContent::revision()` 在任何编辑（含撤销重做）后变化；`TextContent` 的行表可廉价快照（worker 也是这样拿数据）。
- egui 默认 `zoom_with_keyboard = true`，在每帧开始时消费 Ctrl+0 / Ctrl+= / Ctrl+−，视图收不到 Ctrl+0。
- 行号栏宽 40 px，左 20 px 是复制箭头命中区，右 20 px 是行号，没有多余空间。

## Goals / Non-Goals

**Goals:**
- 一个与视图无关的书签集合类型，平移规则单独可测。
- 不改动现有各编辑入口：书签位置靠「内容修订号变化 → 对比前后行表」自行校正。

**Non-Goals:**
- 书签持久化（命名会话、工作区）。
- 十六进制 / 表格视图书签（参考的书签命令只在文本类视图）。
- 无编号书签、书签列表面板、书签间 Next/Previous。

## Decisions

1. **数据模型**（新模块 `bc-gui/src/text/bookmarks.rs`，合并视图复用）：
   `Bookmarks<A> { slots: [Option<A>; 10] }`，`A` 为锚点类型——文本比较 `A = (side, line)`，合并 `A = (pane, row)`。操作：`toggle(n, at)`（实现 spec 的移除 / 移动 / 同行取代规则）、`get(n)`、`clear()`、`at(anchor) -> Option<n>`、`is_empty()`。纯数据，无 egui 依赖，规则全部单测。

2. **编辑后平移 = 修订号驱动的行表比对**：每侧保存 `base: Option<(revision, LineList 快照)>`，仅当该侧有书签时维护。每帧（`ui()` 开头）若该侧 `revision` 与 `base` 不同：
   - 行数不变 → 不平移（覆盖 Replace All、换行符转换等原位修改）。
   - 行数变化 → 求前缀相同行数 `p` 与后缀相同行数 `s`，变化区间 old `[p, old_len-s)` → new `[p, new_len-s)`，增量 `d`。书签行 `< p` 不变；`>= old_len-s` 加 `d`；落在区间内的：若新区间非空则保持相对区间起点的偏移并钳制到新区间末行，若新区间为空（纯删除）则落到 `p`；最后钳制到 `[0, new_len-1]`。
   - 然后更新 `base`。撤销 / 重做同样走这条路，无需额外处理。
   - 备选 A：给每个编辑入口补「起始行 + 行数」——入口多、易漏，放弃。备选 B：用光标位置近似——多点编辑与箭头复制不准，放弃。
   - 比对按行字节比较，复杂度 O(行数)；只在有书签的一侧、且修订号变化的那一帧做一次。

3. **重载 / 换文件 / 交换**：
   - `open_side` 区分来源：重载（同一路径，含手动 Reload 与磁盘变化重载）→ 书签行号钳制到新行数并重置 `base`，不做比对；打开不同文件或剪贴板 → 清除该侧书签。
   - `Swap Sides` → 把每个锚点的 `side` 取反，`base` 随之交换。

4. **绘制**：标记画在所属窗格**文本区右缘**（纵向滚动条左侧）：16×(行高-2) px 圆角徽标，强调色底、白色编号数字，不随横向滚动移动，不占行号栏，因此不会挡住复制箭头（spec「书签显示」）。缩略导航条（`nav_bar.rs`）在书签所在行的位置画 2 px 强调色短刻度。合并视图同样画在锚点窗格的文本区右缘。

5. **命令与菜单**：文本比较 `command()` 解析 `Bookmark N` 子项（Toggle / Go to 的父菜单区分 action_id，例如 `Toggle Bookmark N` / `Go to Bookmark N`），`update_menus` 中：Toggle 子项始终可用；Go to 子项仅当该编号已设置；Clear Bookmarks 仅当非空。Go to：设 `active = side`、编辑器光标到 `(line, 0)`，若显示过滤隐藏该行则沿用 Go To 行号已有的展开逻辑，再 `goto_row`。合并视图在 Search 菜单追加同样三项与快捷键，`Go to` 设 `active = pane` 并 `goto_row(row)`；输出窗格的锚点 `row` 取光标所在输出行的 `out_line(line).row`。

6. **Ctrl+0**：应用启动时 `ctx.options_mut(|o| o.zoom_with_keyboard = false)`。Matcha 一直以 `zoom_factor = 1` 运行（快照也依赖这一点），关闭键盘缩放与现状一致，只是去掉了未记录的 Ctrl+= / Ctrl+− 整体缩放。

## Risks / Trade-offs

- [大文件（百万行）上每次改变行数的编辑都要 O(n) 比对] → 只在该侧有书签时做；按行先比长度再比字节，实际为内存顺扫，百万行约毫秒级；如实测超过 16 ms，再改为只比对编辑前后修订号对应的 `EditStack` 记录。
- [区间内书签的落点是近似（多点编辑导致变化区间很宽时）] → 行数不变的多点编辑完全不平移；行数变化的多点编辑极少与书签同时出现，落点仍在变化区间内、离原位置不远。
- [关闭键盘缩放是全局行为变化] → 已在 proposal 声明；CHANGELOGS 中同步说明。

## Migration Plan

纯新增界面行为；无数据格式变化。回滚撤销视图代码并恢复 `zoom_with_keyboard` 默认值即可。
