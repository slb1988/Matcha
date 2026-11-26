# Proposal

## Why

文本比较与文本合并里经常出现超出窗格宽度的长行（日志、Markdown 段落、压缩配置、中文长句）。目前只能横向滚动阅读，而且两侧一起横移，很难把同一行的前后部分对照着看完。参考工具（BC 5）在 Text Compare 与 Text Merge 中都提供 View › Word Wrap：长行按窗格宽度折行显示，不需要横向滚动。Matcha 的 View 菜单已列出 Word Wrap，但一直是禁用状态（`docs/reference-parity/text-compare.md` 标为「后置：固定行高虚拟化，变高行索引另做」）。

## What Changes

- 文本比较视图与文本合并视图的 View 菜单提供 **Word Wrap** 开关（勾选项），默认关闭，按标签页记录，不写入命名会话或偏好。文本合并的 View 菜单新增该项。
- 开启后：
  - 每个窗格按**自己的**文本区宽度折行（左右宽度不同时各折各的），只影响显示，不改动文件内容、行号与对齐结果。
  - 每个对齐行的显示高度取所有窗格在该行折出的视觉行数的最大值（文本比较 2 个窗格，文本合并 4 个窗格）；行数较少的窗格在该行下方留白，底色、缺失行斜线填充随整行高度铺满，各窗格继续按对齐行逐行对应、同步滚动。
  - 优先在空白处折行；单个词比窗格还宽时在字符处硬折；CJK 字符之间可直接折行。折行宽度按真实字形宽度计算（含中文全角字符与制表符展开）。
  - 横向滚动条隐藏，横向偏移归零；窗格宽度变化（拖动分隔线、调整窗口、切换上下布局）时即时重新折行，并保持视口顶端的对齐行不跳走。
  - 行号、书签标记、行号栏复制箭头只画在该行的首个视觉行；行尾标记（Visible Whitespace / 无行尾）画在最后一个视觉行的末尾。
  - 鼠标单击、拖选、Shift+单击可落在任一视觉行的任一字符；选区与光标按折行位置绘制。
  - 键盘 ↑/↓、Home/End **仍按逻辑行**移动（编辑器光标模型不变）；光标或查找命中位于长行中部时，视图滚动到它所在的视觉行。
  - 单行折出的视觉行数设上限，超过上限时在该行最后一个视觉行末尾显示截断提示，避免超长单行（如几十 MB 的压缩文件）拖垮滚动。
- 文本合并视图取消开启 Word Wrap 时「每行只排版前 4096 字节」的显示截断（行内差异比较仍只看前 4096 字节）。
- 不新增工具栏按钮：参考工具默认工具栏采集（`docs/visual-parity`）中没有 Wrap 按钮。

## Capabilities

### New Capabilities
- `text-word-wrap`：文本比较与文本合并视图中的自动折行：开关、各窗格独立折行宽度、对齐行高取最大值、折行规则、折行状态下的绘制、点击命中、滚动与超长行上限。

### Modified Capabilities
（无。现有 `text-compare-view` 中两侧水平滚动同步、`text-merge-view` 中共享横向滚动条的要求在 Word Wrap 关闭时不变；开启时的覆盖规则写在新 capability 中。）

## Impact

- `crates/bc-gui/src/text/`：`projection.rs`（变高行几何：按显示行的视觉行数前缀和）、`render.rs`（分段绘制、选区/光标/命中、行号栏与行尾标记位置）、`view.rs`（开关、菜单启用、宽度变化与滚动锚定、跳转到视觉行）、`menu.rs`。
- `crates/bc-gui/src/merge/view.rs`：把约 15 处 `scroll.y / row_height` 的固定行高算术改为经共享行几何查询；View 菜单新增 Word Wrap；开启时取消 `LINE_LIMIT` 的显示截断。
- 新增 `bc-gui` 内共享模块（折行断点计算 + 行高前缀和），两视图复用并单测。
- 无新依赖；不改命令行、会话文件格式、报告输出。
- 文档：新增 `docs/features/text-word-wrap.md`，更新 `docs/features/README.md` 索引、`text-compare.md` / `text-merge.md` 链接、`docs/reference-parity/text-compare.md` 与 `text-merge.md` 中 Word Wrap 条目，`crates/bc-gui/README.md` 后置列表，`CHANGELOGS.md`。
