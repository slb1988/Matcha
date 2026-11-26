# 文本自动折行（Word Wrap）

> 文本比较与文本合并视图中按窗格宽度折行显示长行，无需横向滚动即可读完整行。
> 状态：**进行中**（规划完成，实现中）（2026-11-26 更新）。

## 功能简介

- **开关**：View › Word Wrap（勾选项），文本比较与文本合并都有。默认关闭；按标签页记录，
  不写入命名会话或偏好。不提供工具栏按钮（参考工具默认工具栏没有 Wrap 按钮）。
- **只改显示**：文件内容、行号、对齐结果与差异着色都不变。
- **各窗格按自己的宽度折行**：左右（或合并视图的四个窗格）宽度不同时各折各的，任何窗格都不需要横向滚动。
- **对齐行高取最大值**：同一对齐行在所有窗格中等高，高度为各窗格折出的视觉行数的最大值；
  行数较少的窗格在该行下方留白，底色、缺失行斜线、当前行底线铺满整行。
- **折行规则**：优先在空白处折（空白留在上一视觉行末尾）；单词比窗格宽时按字符硬折；
  CJK 字符之间可直接折；按字形实际宽度计算（中文约 2 个 ASCII 宽），制表符按制表宽度展开。
- **行号栏**：行号、书签徽标、复制箭头只画在首个视觉行；行尾标记（Visible Whitespace 的 LF/CRLF/CR、
  无行尾标记）画在最后一个视觉行文字之后。
- **鼠标**：单击、拖选、Shift+单击可落在任一视觉行；点在本窗格该行文字之下的留白处，光标放在行尾。
- **键盘**：↑/↓、PageUp/PageDown、Home/End **仍按逻辑行**移动；光标或查找命中位于长行中部时，
  视图滚到它所在的视觉行。
- **横向滚动**：开启时隐藏横向滚动条、横向偏移归零、横向滚轮无效；关闭后恢复。
- **宽度变化**（拖分隔线、调窗口、切换上下布局、改字号、显隐行号）：立即重新折行，视口顶端的对齐行不跳。
- **超长行上限**：一行在一个窗格中最多 10000 个视觉行，超出部分不显示，末尾以弱色提示
  `… line too long: N more characters not shown`；可在底部 Text Details 中查看完整当前行。

## 代码地图

| 层 | 位置 | 内容 |
|---|---|---|
| 共享模块 | `crates/bc-gui/src/wrap.rs` | `Glyphs`（字宽缓存）、`count` / `breaks`（断点、快路径、`WRAP_CAP`）、`RowHeights`（按显示行的视觉行数前缀和；`build` 预算内精确、超出估算，`refine` / `advance` 补齐） |
| 文本比较：几何 | `crates/bc-gui/src/text/projection.rs` | `Projection::set_heights`：有高度表时 `y_of` / `display_at`（二分）/ `row_height` 按视觉行计；细折叠行仍按自身高度 |
| 文本比较：状态 | `crates/bc-gui/src/text/wrapping.rs` | `WrapState`（每侧 `WrapSpec`、字宽、断点缓存）、`ensure_wrap`（建表 / 重建 / 补齐 / 关闭，滚动锚定 `TopAnchor`）、`wrapped`、`reveal_caret` |
| 文本比较：绘制 | `crates/bc-gui/src/text/render.rs` | 行按 `Piece`（视觉行）排版：可见段、逐段选区、末段行尾标记、截断提示、按段命中；`gutter::paint` 的 `head` 参数（行号、块箭头在首个视觉行） |
| 文本比较：入口 | `crates/bc-gui/src/text/view.rs` | `word_wrap` 开关（View › Word Wrap 勾选）；`ui()` 先算窗格矩形 → `ensure_wrap` → 落实 `pending_scroll` / `pending_reveal`；开启时 `scroll.x = 0` |
| 文本合并：几何 | `crates/bc-gui/src/merge/view.rs` | `heights`（按网格行）与 `grid_y` / `grid_at` / `grid_units`：所有行位置都经它们；View 菜单 Word Wrap；开启时 `max_x = 0`（不显示共用横向滚动条）、不受 `LINE_LIMIT` 显示截断；`paint_gutter` 的 `head`；`line_job` 的段起点与起始列 |
| 文本合并：状态 | `crates/bc-gui/src/merge/view/wrapping.rs` | `MergeWrap`（四窗格 `WrapSpec`、字宽、断点缓存；网格代号 + 四内容修订号失效）、`ensure_wrap`、`pane_line`、`wrapped`、`reveal_caret`、滚动锚定 |
| 文本合并：模型 | `crates/bc-gui/src/merge/model.rs` | `grid_generation()`：网格重建计数 |

## 关键语义

- 行高按「视觉行」整数单位累加，像素只在最终换算时乘以行高。
- Word Wrap 关闭时走原固定行高路径，画面逐像素不变。
- 窗格文本区宽度 = 窗格宽 − 纵向滚动条 18 − 行号栏 41（无行号时 8）− 右侧留白 4；断点判断再扣 0.5 px 余量。
- 每次编辑都会重建投影，高度表随之重建（≤ 16 MB 需扫描的字节同步精确，其余先估算、视口附近当帧精确、
  空闲帧每帧约 2 MB 补齐）；重建前后按「顶端显示行 + 行内偏移」锚定，视口顶端行不跳。
- 光标随键盘 / 编辑移动或查找命中后，按光标所在视觉行做最小滚动（`reveal_caret`）；关闭 Word Wrap 时沿用原
  「离开 30 行窗口才滚动」规则。
- 鼠标松开时编辑器重新取得焦点后，同时恢复方向键锁定（否则 egui 会把下一次方向键当作焦点导航，
  在折行的高行下方找到别的控件并转走焦点）。

## 使用与验证

- 单元测试：`cargo test -p bc-gui --lib wrap::`、`cargo test -p bc-gui --lib text::projection`。
- 文本比较视图测试：`cargo test -p bc-gui --lib word_wrap`（开关与按标签页、横向滚动隐藏、各侧独立宽度与行高取最大、
  缺失行与行尾标记、点击 / 留白 / 拖选、宽度变化与开关时顶端行不变、↑/↓ 按逻辑行与光标 / 查找可见、输入当帧重新折行）。
- 文本合并视图测试：`cargo test -p bc-gui --lib merge_word_wrap`（菜单与共用横向滚动条、四窗格行高取最大与对齐、
  输入窗格第二视觉行点击、输出窗格输入当帧重新折行、光标与 Find Next 可见、调整列宽时顶端行不变）。
- 手动：`matcha /solo a.txt b.txt` → View › Word Wrap；拖动中间分隔线观察两侧各自折行。

## 深入阅读

- 变更：`openspec/changes/text-word-wrap/`（proposal / design D1–D7 / spec）
- 参考帮助：本地 CHM 解包 `tmp/hex-ref/chm/viewtext.html`（Word Wrap 段落）
- 文本比较总览：[text-compare.md](text-compare.md)；文本合并：[text-merge.md](text-merge.md)
