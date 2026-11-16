# Tasks

## 1. 功能文档先行

- [x] 1.1 在 `docs/features/text-compare.md` 的「功能简介」与「关键语义」中加入"行号栏复制箭头"：区块箭头（整块覆盖）、选区箭头（选中行覆盖，外观不同）、只读/对齐中禁用；验收：文档描述与 `specs/text-compare-view/spec.md` 的三个新需求一一对应

## 2. 箭头判定与复制执行（view.rs）

- [x] 2.1 新增 `GutterArrow { kind: Section | Selection, disabled: Option<String> }` 与 `TextCompareView::gutter_arrow(s, row, selected: Option<Range<usize>>)`，按 design 决定 1 判定种类与禁用原因；在 `view_tests.rs` 加单测覆盖：差异行→Section、选区内相同行→Selection、选区外差异行→Section、相同行无箭头、空隙侧无箭头、目标只读→disabled、`map.is_stale()`→disabled；`cargo test -p bc-gui gutter_arrow` 通过
- [x] 2.2 从 `copy_to` 无选区分支提取 `copy_section_at(destination, row)`，`copy_to` 改为调用它；新增 `apply_gutter_copy(s, row, kind)`：执行前再查禁用条件，Section 走 `copy_section_at`，Selection 走 `copy_lines` 后清除来源侧 anchor，并写成功/失败消息；单测验证左→右整块覆盖后区块变为相同、一次 Undo 复原，部分选区只覆盖对齐行且选区被清除；现有 `copy_to` 相关测试仍通过

## 3. 绘制与命中（gutter.rs / render.rs）

- [x] 3.1 `gutter.rs`：新增 `arrow_rect(viewport_rect)`；`paint` 改为接收 `Option<GutterArrow>` 与 `hovered`，绘制黄色区块箭头、蓝色带竖线的选区箭头、灰色禁用箭头和悬停高亮；去掉原来基于 `difference` 布尔值的分支；`dev-build.ps1` 编译通过
- [x] 3.2 `render.rs::pane`：每帧先算一次本侧选区行区间；对有箭头的可见行用 `ui.interact` 注册箭头命中区（在行分配之后），设置悬停提示与手形光标，点击时记录 `pending_gutter_copy`；原始指针放光标/拖选/align-with 分支和双击分支排除箭头命中区；ScrollArea 结束后执行 `apply_gutter_copy`；`dev-build.ps1` 编译通过
- [x] 3.3 在 `view_tests.rs` 用现有 `frame`/`click` 原始输入辅助函数加端到端测试：点击左侧差异行箭头位置后右侧内容被覆盖且左侧光标未移动；右侧仅右有行的区块点击后插入左侧；选中两行后点击选区箭头只覆盖这两行；右侧只读时点击不改内容；编辑后对齐未完成时点击不改内容；`cargo test -p bc-gui` 全部通过

## 4. 集成检查与记录

- [x] 4.1 运行 `cargo test -p bc-gui -p bc-engine-text` 与 `cargo clippy -p bc-gui`，确认无新增失败或告警；用 `target\debug\matcha.exe a.cpp b.cpp` 实机点击区块箭头和选区箭头，确认外观可区分、悬停提示正确、Ctrl+Z 可撤销
- [x] 4.2 在 `CHANGELOGS.md` 顶部记录本次变更（行号栏箭头可点击：区块整块覆盖 + 选区箭头覆盖选中行）；验收：条目存在且与 feature 文档一致

## Workflow follow-up

- 评审通过后执行 `/opsx:archive text-gutter-copy-arrows`，把 delta 合并进 `openspec/specs/text-compare-view/spec.md`。
