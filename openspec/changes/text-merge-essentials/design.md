# Design

## Context

- `MergeView::pane`（`merge/view.rs`）逐显示行绘制四个窗格，已调用文本比较的 `text::render::layout`，但传入的 `tokens` 与 `spans` 都是空切片、`LineInk.syntax` 为默认值——所以语法与行内高亮只差「把数据传进去」。
- `MergeModel` 持有 `map: TextMap`（`left/right/center: TextContent`，三方行表）与 `output: TextContent`；四份内容载入时都带了文件格式（`model.rs` 用格式库按 base 名解析），`TextContent::tokens(line)` 可直接取语法 token。
- 行内差异：文本比较用 `inline_diff_classified(LineView, LineView, &CompiledComparison, &InlineOptions)`，按行缓存（`ensure_inline`，上限 512 条后清空）。
- 查找：`FindPanel`（`text/find_panel.rs`）是独立的 egui 组件，输出 `FindAction`；引擎 `bc_engine_text::search::{find_next, replace_all}` 按 `TextContent` 工作，不回绕。
- 滚动：四窗格共用 `scroll: Vec2`；横向只能滚轮（Shift+滚轮），已有 `content_width` 统计；`chrome::scrollbar::show_horizontal_px` 现成。
- 输出窗格编辑一律经 `MergeModel::edit(editor, |editor, content| …)`，它负责撤销分组与把受影响行标为 `OutOrigin::Manual`。
- 输入窗格只有点击定位（`current_row`），没有光标 / 选区。

## Goals / Non-Goals

**Goals:**
- 最大化复用文本比较的绘制、查找与行内差异代码，合并视图只做「取数据 + 接线」。
- 输入窗格的只读语义不变（spec「四窗格布局」）。

**Non-Goals:**
- base 窗格与输出窗格的行内高亮（输出按来源着色已足够区分；base 与哪一侧比对存在歧义）。
- 输入窗格编辑、从输入窗格选区 Take 选中行、显示过滤（Show Conflicts 等）、会话内改规则、自动折行。
- 超长行的横向窗口化（仍只排版每行前 4096 字节，与现状一致）。

## Decisions

1. **行内差异缓存在 `MergeView`**：`inline: HashMap<(pane, row), InlineDiff>`，只为可见且需要的行计算（左 / 右窗格、该行属于改动或冲突、本侧与 base 都有内容）。比较规则用合并会话的规则编译出的 `CompiledComparison`（与载入时一致，含 ignore-unimportant），缓存超过 512 项时清空。输入窗格只读，缓存无需失效；`Take` / 输出编辑不影响输入窗格，也不清缓存。
   - 着墨：`layout` 现有的 `DiffColors` 按重要性取色。新增一个构造参数让重要差异色可由调用方指定（左窗格 `MERGE_LEFT`、右窗格 `MERGE_RIGHT`、冲突区段 `MERGE_CONFLICT`），不重要差异沿用 `DiffColors` 的不重要色。文本比较的调用点传原值，行为不变。
   - 备选：在引擎里做三方逐字符合并标注——工作量大且只服务一个视图，放弃。

2. **语法着色**：每个窗格从对应 `TextContent` 取 `tokens(line)`，`LineInk.syntax` 用文本比较同一个 `SyntaxStyle::for_format(...)`（按格式名取覆盖），`bold` 用 `theme::editor_bold`。`show_syntax: bool`（默认 true）关闭时传空 token。新增 View 菜单只含 Syntax Highlighting 一个勾选项。

3. **输入窗格的光标与选区用 `Editor`**：`input_editors: [Editor; 3]`，对 `map.left/center/right` 只调用定位 / 选择 / 取选区文本的方法，从不调用插入删除；键盘事件在输入窗格活动时只处理方向键、Shift 选区、Ctrl+A、Ctrl+C，其余（字符、Backspace、Delete、Enter、粘贴、IME）直接丢弃。命中计算复用输出窗格现有的 `galley.cursor_from_pos` 逻辑，抽成一个对四窗格通用的私有函数。
   - 行号 ↔ 对齐行：输入侧行号到 `row` 用 `TextMap` 行表一次线性构建的 `Vec<usize>` 索引（每侧一个，载入后不变），避免每次查找扫描全表。

4. **查找接入**：`find: FindPanel` 放在工具栏下方（与文本比较相同位置）。`FindAction::Next/Previous` → 在活动窗格的 `TextContent` 上 `find_next(from = 活动窗格光标)`，命中后设置该窗格选区、`current_row = 行→row`、`goto_row`。「仅在改动中」传入的 `allowed` 区间 = 该窗格在改动 / 冲突区段内的行区间（由 `MergeState` 的区段与 `map` 行表推出）。
   - 替换只在 `active == OUT`：`Replace` = 若当前选区等于上次命中则替换后查下一个；`All` = 通过新增 `MergeModel::replace_all(editor, options, replacement, allowed)`，内部在 `edit` 闭包里调用 `search::replace_all`，从而撤销为一组、受影响行标手工、冲突解决计数按既有规则更新。输入窗格活动时 `replace_open` 强制为 false。

5. **横向滚动条**：在 `body` 底部预留 16 px 一条，横跨四窗格区（与右侧纵向滚动条相接），上限沿用现有 `content_width - 最窄列宽 + GUTTER + 40` 的计算；`content_width` 只统计本帧可见行，因此需要内容「放不下」时才显示，放得下时偏移归零（spec）。

6. **右键菜单**：输入窗格新增 `input_context_items(pane)`：Take Left / Center / Right（`section_at(row)` 有值时可用）、分隔、Copy（有选区）、Select All。右键先把 `current_row` 设为所点行、活动窗格设为该窗格，然后弹出。

## Risks / Trade-offs

- [行内差异在大量改动的大文件上首次滚动时卡顿] → 只算可见行，单行限 4096 字节（与绘制截断一致），缓存命中后零开销。
- [`layout` 签名变动影响文本比较] → 新参数以「重要差异色覆盖：Option」形式加入，文本比较传 `None`；文本比较的快照测试必须保持逐像素一致作为回归门禁。
- [输入窗格吞掉编辑键可能与全局快捷键冲突] → 只丢弃「会修改内容」的键，Ctrl+L/R/B、Ctrl+N/P、F3 等命令键照常走 `keyboard()`。
- [合并的快照场景变化] → `merge-basic`、`merge-conflict` 快照会因语法与行内着色改变，需要重新生成并人工核对一次。

## Migration Plan

纯 GUI 行为增强，无数据格式变化；回滚即撤销视图代码。
